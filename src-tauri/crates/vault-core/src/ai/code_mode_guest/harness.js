// PathKeep code-mode guest harness — W-AI-8 WU-1 (D7 / 02 §G Layer 2).
//
// THIS IS THE JAVASCRIPT SOURCE FOR THE COMMITTED `harness.wasm` BUILD ARTIFACT.
// It is NOT compiled at app build time — it is pre-compiled once with the `javy` CLI (a DEV tool)
// into `harness.wasm`, whose SHA-256 is pinned + asserted at load in `code_mode.rs`. Production
// does NOT need `javy` installed. See `code_mode.rs` "## Guest engine status" for the regen task.
//
// ## What this harness does (the host<->guest contract)
// Javy compiles QuickJS-in-WASM. The ONLY thing the guest can reach is WASI fd I/O (stdin/stdout) —
// it has ZERO ambient authority (no fs/net/real-clock/env; see the scoped WasiCtx in code_mode.rs).
// So every host interaction is a length-prefixed JSON RPC frame over stdio, serviced SYNCHRONOUSLY
// by the host:
//   - guest WRITES a request frame to stdout (fd 1)
//   - guest READS the reply frame from stdin (fd 0); the host computes the reply during that read
// A frame is: 4-byte little-endian length prefix + UTF-8 JSON body.
//
// The harness:
//   1. fetches the LLM's JS SOURCE from the host via the `source` op (NOT over a one-shot stdin —
//      stdin is the reply channel, so the source arrives as an RPC reply).
//   2. exposes `query_history(argsObj)` and `fetch_visits(ids)` as globals wired to the host's
//      read-only retrieval (each is one `query_history` / `fetch_visits` RPC op).
//   3. `eval`s the source (as a Function body, so `return value;` works) with those globals in scope.
//   4. serializes the returned value to JSON and hands it to the host via the `result` op.
//   5. reports any thrown JS error (syntax/runtime) to the host via the `error` op so the host can
//      surface an honest CodeOutcome.error rather than the guest crashing opaquely.

const STDIN = 0;
const STDOUT = 1;

function writeAll(fd, bytes) {
  let offset = 0;
  while (offset < bytes.length) {
    const written = Javy.IO.writeSync(fd, bytes.subarray(offset));
    if (written <= 0) {
      break;
    }
    offset += written;
  }
}

function readExactly(fd, count) {
  const out = new Uint8Array(count);
  let got = 0;
  while (got < count) {
    const scratch = new Uint8Array(count - got);
    const read = Javy.IO.readSync(fd, scratch);
    if (read === 0) {
      break; // host closed the channel (e.g. a refused/cancelled call)
    }
    out.set(scratch.subarray(0, read), got);
    got += read;
  }
  return out.subarray(0, got);
}

function encodeU32LE(value) {
  const bytes = new Uint8Array(4);
  bytes[0] = value & 0xff;
  bytes[1] = (value >> 8) & 0xff;
  bytes[2] = (value >> 16) & 0xff;
  bytes[3] = (value >>> 24) & 0xff;
  return bytes;
}

function readU32LE() {
  const bytes = readExactly(STDIN, 4);
  if (bytes.length < 4) {
    return -1; // channel closed before a full length prefix → host refused the call
  }
  return bytes[0] | (bytes[1] << 8) | (bytes[2] << 16) | bytes[3] * 0x1000000;
}

// One synchronous request/reply round-trip with the host. Returns the parsed reply object, or
// `null` when the host refused the call (closed the channel) — the caller decides how to degrade.
function rpc(request) {
  const body = new TextEncoder().encode(JSON.stringify(request));
  writeAll(STDOUT, encodeU32LE(body.length));
  writeAll(STDOUT, body);
  const length = readU32LE();
  if (length < 0) {
    return null;
  }
  const reply = readExactly(STDIN, length);
  if (reply.length < length) {
    return null;
  }
  return JSON.parse(new TextDecoder().decode(reply));
}

function main() {
  const sourceReply = rpc({ op: "source" });
  // The host always answers the source op first; a null here means the channel never opened.
  const source = sourceReply && typeof sourceReply.source === "string" ? sourceReply.source : "";

  // The capability-scoped host API, exposed as globals the LLM's JS calls directly. A refused call
  // (host-call budget exhausted / cancelled) surfaces as a thrown Error so the script stops cleanly
  // and the host returns its partial result + the recorded limit marker.
  globalThis.query_history = function (args) {
    const reply = rpc({ op: "query_history", args: args || {} });
    if (reply === null) {
      throw new Error("query_history refused by host");
    }
    return reply;
  };
  globalThis.fetch_visits = function (ids) {
    const reply = rpc({ op: "fetch_visits", args: { ids: ids || [] } });
    if (reply === null) {
      throw new Error("fetch_visits refused by host");
    }
    return reply;
  };

  // eval the LLM source as a function body so a top-level `return` distills a value.
  // A syntax error here throws synchronously and is caught below.
  const program = new Function(source);
  const value = program();
  rpc({ op: "result", value: value === undefined ? null : value });
}

try {
  main();
} catch (error) {
  // Honest error reporting: a JS syntax/runtime error or a host-refused call becomes a `result`
  // with an error string (the host maps it to CodeOutcome.error) rather than an opaque guest crash.
  const message = error && error.message ? String(error.message) : String(error);
  rpc({ op: "error", message: message });
}
