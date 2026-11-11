#!/usr/bin/env python3
"""Verify the built agent-history CLI without exposing native conversation data.

Responsibilities: run real CLI/file/database/index/export paths, measure their cost,
and publish aggregate evidence. Not responsible for building the CLI or claiming
14.4M/other-platform performance from a smaller run.

Examples (run only during final verification):
  python3 scripts/agent-history-verify.py --synthetic-records 10000
  python3 scripts/agent-history-verify.py --local-samples

Local mode reads at most two stable, <=8 MiB native transcripts per harness from
the default Claude Code, Codex, pi and DeepSeek roots. It skips symlinks and files
modified within the last hour. It preserves their layout in private 0700 temporary
storage, passes ONLY those copied roots to the CLI, and checks original bytes and
metadata again afterwards. Missing attachment/lineage diagnostics are expected
when intentionally sampling; they are recorded as codes, never hidden as complete
coverage. This does not validate all accounts, desktop stores or custom roots.

Passwords use stdin. CLI stdout/stderr, native paths, titles, IDs, queries derived
from private content and per-file content hashes are never logged or included in
the report. No networking or harness executable is used. Raw copies, archives,
exports and timing files are deleted on normal exit, errors and handled signals.
SIGKILL, host crashes and filesystem cleanup failures can leave a private temp
directory; cleanup cannot promise secure erasure from SSD snapshots or swap.

Synthetic input is streamed Pi v3 with actual user/assistant/toolResult envelopes,
Unicode and an end-of-stream search marker. Measurements include password unwrap
and process startup. RSS is per command via optional /usr/bin/time, not Python's
cumulative child-process high-water mark. Reports contain only aggregate evidence
at artifacts/agent-history/{real-samples,scale}.json, including failed runs.
"""

from __future__ import annotations

import argparse
import collections
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import secrets
import shutil
import signal
import stat
import subprocess
import sys
import tempfile
import time


REPO = Path(__file__).resolve().parents[1]
MAX_SAMPLE_BYTES = 8 * 1024 * 1024
BUFFER_BYTES = 256 * 1024
NATIVE_SUFFIXES = (".jsonl", ".jsonl.zst", ".jsonl.zstd")
TAIL = "pathkeepverificationtailquartz"
BASE_MS = 1_794_312_000_000


class VerificationFailure(Exception):
    """A fixed diagnostic code, never an exception containing native file content."""


def require(condition: bool, code: str) -> None:
    if not condition:
        raise VerificationFailure(code)


def fingerprint(value: os.stat_result) -> tuple[int, int, int, int]:
    return value.st_dev, value.st_ino, value.st_size, value.st_mtime_ns


def open_readonly(path: Path, strict_parents: bool = False):
    flags = os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0)
    if strict_parents and os.name == "posix":
        parent = os.open(path.anchor, os.O_RDONLY | os.O_DIRECTORY)
        try:
            for component in path.parts[1:-1]:
                next_parent = os.open(component, flags | os.O_DIRECTORY, dir_fd=parent)
                os.close(parent)
                parent = next_parent
            descriptor = os.open(path.name, flags, dir_fd=parent)
        finally:
            os.close(parent)
    else:
        if strict_parents:
            require(not any(parent.is_symlink() for parent in path.parents), "source_parent_symlink")
        descriptor = os.open(path, flags)
    try:
        require(stat.S_ISREG(os.fstat(descriptor).st_mode), "source_not_regular")
        return os.fdopen(descriptor, "rb", buffering=0)
    except BaseException:
        os.close(descriptor)
        raise


def digest_file(path: Path) -> str:
    digest = hashlib.sha256()
    with open_readonly(path) as source:
        while block := source.read(BUFFER_BYTES):
            digest.update(block)
    return digest.hexdigest()


def digest_original(path: Path, expected: tuple[int, int, int, int]) -> str:
    """Hash exactly the selected source size; a resumed writer cannot extend our read."""
    digest = hashlib.sha256()
    with open_readonly(path, strict_parents=True) as source:
        require(fingerprint(os.fstat(source.fileno())) == expected, "original_source_metadata_changed")
        remaining = expected[2]
        while remaining:
            block = source.read(min(BUFFER_BYTES, remaining))
            require(bool(block), "original_source_short_read")
            digest.update(block)
            remaining -= len(block)
        require(fingerprint(os.fstat(source.fileno())) == expected, "original_source_metadata_changed")
    require(fingerprint(path.lstat()) == expected, "original_source_replaced")
    return digest.hexdigest()


def bytes_under(root: Path) -> int:
    return sum(path.stat().st_size for path in root.rglob("*") if path.is_file())


def code_counts(issues: list[dict]) -> dict[str, int]:
    # Only allow the generated code shape; path/detail/ordinal remain private.
    codes = [issue.get("code", "invalid_diagnostic") for issue in issues]
    return dict(collections.Counter(
        code if isinstance(code, str) and re.fullmatch(r"[A-Za-z0-9_-]{1,100}", code)
        else "invalid_diagnostic" for code in codes
    ))


def build_fingerprint(binary: Path) -> dict:
    """Identify actual inputs even when an offloaded source archive omits Git metadata."""
    def git(*arguments: str) -> bytes:
        result = subprocess.run(["git", *arguments], cwd=REPO, capture_output=True, check=True)
        return result.stdout

    provenance = {"git_head": None, "git_dirty": None, "git_status_sha256": None,
                  "git_provenance": "unavailable", "source_archive": not (REPO / ".git").exists()}
    # Without a local marker, Git could accidentally discover an unrelated parent
    # checkout. An archive supplies no evidence about its original HEAD or dirtiness.
    if not provenance["source_archive"]:
        try:
            head = git("rev-parse", "HEAD").decode("ascii").strip()
            status = git("status", "--porcelain=v1", "-z", "--untracked-files=normal")
        except (OSError, subprocess.CalledProcessError, UnicodeError):
            pass
        else:
            provenance.update({"git_head": head, "git_dirty": bool(status),
                               "git_status_sha256": hashlib.sha256(status).hexdigest(),
                               "git_provenance": "available"})
    source_hash = hashlib.sha256()
    for relative in ("src-tauri/crates/agent-history", "src-tauri/crates/vault-store",
                     "src-tauri/crates/vault-crypto"):
        for path in sorted((REPO / relative).rglob("*")):
            if path.is_file() and path.suffix in (".rs", ".toml"):
                source_hash.update(str(path.relative_to(REPO)).encode())
                source_hash.update(path.read_bytes())
    for relative in ("src-tauri/Cargo.toml", "src-tauri/Cargo.lock", ".cargo/config.toml"):
        path = REPO / relative
        if path.is_file():
            source_hash.update(relative.encode())
            source_hash.update(path.read_bytes())
    return {**provenance,
            "source_tree_sha256": source_hash.hexdigest(),
            "binary_sha256": digest_file(binary), "binary_bytes": binary.stat().st_size,
            "fingerprint_note": "Records source and binary independently; not proof of build provenance."}


class Runner:
    """CLI responses are private in-memory values; only measurements leave this class."""

    def __init__(self, binary: Path, private: Path, report: dict, timeout: int):
        self.binary, self.private, self.report, self.timeout = binary, private, report, timeout
        self.archive = private / "archive"
        self.password = secrets.token_urlsafe(32).encode()
        self.temp = private / "temp"
        self.temp.mkdir(mode=0o700)

    def call(self, command: str, *arguments: str, operation: str | None = None) -> dict:
        operation = operation or command.replace("-", "_")
        self.report["phase"] = operation
        argv = [str(self.binary), command, *map(str, arguments)]
        if command != "plan":
            argv.extend(["--archive", str(self.archive), "--password-stdin"])
        timer = self.private / "command-time.txt"
        timed = Path("/usr/bin/time").is_file() and sys.platform in ("darwin", "linux")
        if timed:
            argv = ["/usr/bin/time", "-l" if sys.platform == "darwin" else "-v",
                    "-o", str(timer), *argv]
        environment = os.environ.copy()
        environment.update({"TMPDIR": str(self.temp), "TMP": str(self.temp), "TEMP": str(self.temp)})
        started = time.perf_counter()
        process = subprocess.Popen(argv, cwd=self.private, env=environment, stdin=subprocess.PIPE,
                                   stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                   start_new_session=os.name == "posix")
        try:
            stdout, stderr = process.communicate(self.password, timeout=self.timeout)
        except BaseException:
            try:
                if os.name == "posix":
                    os.killpg(process.pid, signal.SIGKILL)
                else:
                    process.kill()
            except ProcessLookupError:
                pass
            process.communicate()
            raise
        measurement = {"operation": operation, "wall_seconds": round(time.perf_counter() - started, 6),
                       "max_rss_bytes": None, "exit_code": process.returncode,
                       "stderr_bytes": len(stderr),
                       "rss_method": ("time_l_bytes" if sys.platform == "darwin" else "time_v_kib") if timed else None}
        if timed and timer.exists():
            timing = timer.read_text(errors="replace")
            pattern = (r"^\s*(\d+)\s+maximum resident set size\s*$" if sys.platform == "darwin"
                       else r"^\s*Maximum resident set size \(kbytes\):\s*(\d+)\s*$")
            match = re.search(pattern, timing, re.MULTILINE)
            if match:
                measurement["max_rss_bytes"] = int(match[1]) * (1 if sys.platform == "darwin" else 1024)
            timer.unlink()
        self.report["measurements"].append(measurement)
        try:
            result = json.loads(stdout)
        except (ValueError, UnicodeError):
            raise VerificationFailure("cli_response_not_json") from None
        require(isinstance(result, dict), "cli_response_not_object")
        if process.returncode != 0:
            self.report["cli_error_codes"] = code_counts([result.get("error", {})])
            raise VerificationFailure("cli_command_failed")
        return result


def native_files(root: Path, errors: collections.Counter):
    def failed(_error):
        errors["enumeration_failed"] += 1

    if root.is_symlink() or not root.is_dir():
        errors["native_subtree_unavailable"] += 1
        return
    for folder, directories, filenames in os.walk(root, followlinks=False, onerror=failed):
        directories[:] = sorted(name for name in directories if not (Path(folder) / name).is_symlink())
        for name in sorted(filenames):
            if name.endswith(NATIVE_SUFFIXES):
                yield Path(folder) / name


def select_samples(home: Path) -> tuple[dict[str, list[tuple[Path, Path]]], dict]:
    layouts = {
        "claude-code": (home / ".claude", [("projects", "main", 1), ("projects", "subagent", 1)]),
        "codex": (home / ".codex", [("sessions", "active", 1), ("archived_sessions", "archived", 1)]),
        "pi": (home / ".pi/agent", [("sessions", "all", 2)]),
        "deepseek": (home / ".dsh", [("sessions", "all", 2)]),
    }
    selected, summary = {}, {}
    cutoff = time.time_ns() - 3_600_000_000_000
    for harness, (root, categories) in layouts.items():
        errors = collections.Counter()
        choices = []
        if not root.is_dir() or root.is_symlink():
            summary[harness] = {"status": "skipped", "reason": "default_root_unavailable", "selected_files": 0}
            continue
        for subtree, category, count in categories:
            candidates = []
            for index, path in enumerate(native_files(root / subtree, errors)):
                if index >= 100_000:
                    errors["enumeration_limit"] += 1
                    break
                relative = path.relative_to(root)
                is_child = "subagents" in relative.parts
                if (category == "main" and is_child) or (category == "subagent" and not is_child):
                    continue
                try:
                    metadata = path.lstat()
                except OSError:
                    errors["metadata_unavailable"] += 1
                    continue
                if (not stat.S_ISREG(metadata.st_mode) or metadata.st_mtime_ns > cutoff
                        or not 0 < metadata.st_size <= MAX_SAMPLE_BYTES):
                    continue
                preference = int(harness == "codex" and not path.name.endswith(".jsonl.zst"))
                candidates.append(((preference, -metadata.st_mtime_ns, str(relative)), path))
                candidates.sort(key=lambda item: item[0])
                del candidates[count:]
            choices.extend((root, path) for _, path in candidates)
        selected[harness] = choices
        summary[harness] = {"status": "selected" if choices else "skipped",
                            "selected_files": len(choices), "scan_issue_codes": dict(errors)}
        if not choices:
            summary[harness]["reason"] = "no_bounded_stable_native_file"
    return selected, summary


def copy_sample(original: Path, destination: Path) -> dict:
    destination.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
    digest = hashlib.sha256()
    with open_readonly(original, strict_parents=True) as source, destination.open("xb") as target:
        before = os.fstat(source.fileno())
        require(0 < before.st_size <= MAX_SAMPLE_BYTES, "sample_size_changed")
        require(before.st_mtime_ns <= time.time_ns() - 3_600_000_000_000, "sample_became_active")
        remaining = before.st_size
        while remaining:
            block = source.read(min(BUFFER_BYTES, remaining))
            require(bool(block), "sample_short_read")
            target.write(block)
            digest.update(block)
            remaining -= len(block)
        require(fingerprint(before) == fingerprint(os.fstat(source.fileno())), "sample_changed_during_copy")
    require(fingerprint(before) == fingerprint(original.lstat()), "sample_replaced_during_copy")
    expected = digest.hexdigest()
    require(digest_original(original, fingerprint(before)) == expected, "sample_changed_during_copy")
    return {"original": original, "copied": destination, "fingerprint": fingerprint(before),
            "sha256": expected, "bytes": before.st_size}


def generate_pi(root: Path, records: int) -> Path:
    path = root / "sessions/--synthetic-project--/2026-11-10T12-00-00Z_scale.jsonl"
    path.parent.mkdir(mode=0o700, parents=True)
    with path.open("x", encoding="utf-8", newline="\n") as output:
        def emit(value):
            output.write(json.dumps(value, ensure_ascii=False, separators=(",", ":")) + "\n")

        emit({"type": "session", "version": 3, "id": "pathkeep-scale-fixture",
              "timestamp": "2026-11-10T12:00:00.000Z", "cwd": "/synthetic/project"})
        previous = None
        cached_second, timestamp_prefix = None, ""
        for index in range(records):
            identifier, phase = f"event-{index}", index % 4
            second, milliseconds = divmod(BASE_MS + index, 1000)
            if second != cached_second:
                timestamp_prefix = datetime.fromtimestamp(second, timezone.utc).strftime("%Y-%m-%dT%H:%M:%S")
                cached_second = second
            text = f"Record {index}: 歷史備份 Café Δ local evidence and tool output."
            if index == records - 1:
                text += " " + TAIL
            message = {"role": "user", "content": [{"type": "text", "text": text}],
                       "timestamp": BASE_MS + index}
            if phase in (1, 3):
                message.update({"role": "assistant", "api": "anthropic-messages",
                                "provider": "synthetic", "model": "synthetic-model",
                                "stopReason": "toolUse" if phase == 1 else "stop",
                                "usage": {"input": 10, "output": 2, "cacheRead": 0,
                                          "cacheWrite": 0, "totalTokens": 12,
                                          "cost": {"input": 0, "output": 0, "cacheRead": 0,
                                                   "cacheWrite": 0, "total": 0}}})
                if phase == 1:
                    message["content"].append({"type": "toolCall", "id": f"call-{index // 4}",
                                               "name": "read", "arguments": {"path": "fixture.txt"}})
            elif phase == 2:
                message.update({"role": "toolResult", "toolCallId": f"call-{index // 4}",
                                "toolName": "read", "isError": False})
            emit({"type": "message", "id": identifier, "parentId": previous,
                  "timestamp": f"{timestamp_prefix}.{milliseconds:03d}Z", "message": message})
            previous = identifier
    return path


def require_synthetic_search_coverage(records: int, tail: dict, unicode: dict) -> None:
    """Every native event has the Unicode phrase; one has the tail, the header has neither."""
    require(type(tail.get("total")) is int and tail["total"] == 1
            and tail.get("complete") is True, "synthetic_tail_not_searchable")
    require(type(unicode.get("total")) is int and unicode["total"] == records
            and unicode.get("complete") is True, "synthetic_unicode_not_searchable")


def exercise(runner: Runner, roots: dict[str, Path], copied: list[dict], synthetic: bool) -> None:
    require(bool(roots), "no_explicit_sources")  # Empty CLI sources would select the actual home.
    arguments = [value for harness, root in roots.items() for value in ("--source", f"{harness}={root}")]
    plan = runner.call("plan", *arguments)
    require(not plan.get("truncated", False), "sample_plan_truncated")
    require(len(plan["sources"]) == len(copied), "sample_plan_inventory_mismatch")
    runner.report["plan_issue_codes"] = code_counts(plan.get("issues", []))
    runner.call("init")
    backup = runner.call("backup", *arguments, "--execute", operation="backup_initial")
    runner.report.update({"backup_status": backup["status"], "issue_codes": code_counts(backup["issues"]),
                          "captured_files": backup["captured"], "source_bytes": sum(item["bytes"] for item in copied)})
    require(backup["captured"] == len(copied), "not_every_sample_was_captured")
    if synthetic:
        require(backup["status"] == "complete", "synthetic_coverage_partial")
    sessions = runner.call("list", "--limit", "100")
    require(sessions["next_cursor"] is None, "unexpected_sample_session_pagination")
    runner.report["parsed_sessions_by_harness"] = dict(collections.Counter(item["harness"] for item in sessions["items"]))
    if synthetic:
        require(len(sessions["items"]) == 1, "synthetic_session_missing")
    search = runner.call("search", "--query", TAIL if synthetic else "the", "--limit", "5")
    runner.report["search"] = {"matches": search["total"], "complete": search["complete"],
                               "private_query_used": False}
    if synthetic:
        unicode_search = runner.call("search", "--query", "歷史備份", "--limit", "5", operation="search_unicode")
        runner.report["search_acceptance"] = {
            "scope": "native events; session header excluded",
            "tail": {"expected": 1, "actual": search["total"], "complete": search["complete"]},
            "unicode": {"expected": runner.report["records"], "actual": unicode_search["total"],
                        "complete": unicode_search["complete"]},
        }
        require_synthetic_search_coverage(runner.report["records"], search, unicode_search)
    copied_by_path = {str(item["copied"]): item for item in copied}
    for index, source in enumerate(plan["sources"]):
        revisions = runner.call("revisions", "--source-id", source["source_id"], "--limit", "2")
        require(len(revisions["items"]) == 1 and revisions["next_cursor"] is None, "initial_revision_count")
        destination = runner.private / f"export-{index}.raw"
        runner.call("raw-export", "--revision", str(revisions["items"][0]["id"]),
                    "--output", str(destination), "--execute")
        expected = copied_by_path[str(Path(source["path"]))]
        require(digest_file(destination) == expected["sha256"], "raw_export_hash_mismatch")
        destination.unlink()
    runner.report["raw_integrity"] = {"checked_files": len(plan["sources"]), "all_sha256_equal": True}
    replay = runner.call("backup", *arguments, "--execute", operation="backup_unchanged")
    require(replay["captured"] == 0 and replay["unchanged"] == len(copied), "reopen_not_idempotent")
    runner.report["reopen_idempotent"] = True
    verification = runner.call("verify")
    require(verification["status"] == "complete", "archive_integrity_failed")
    runner.report["archive_integrity"] = "complete"
    runner.report["archive_bytes"] = bytes_under(runner.archive)


def verify_originals(copied: list[dict], report: dict) -> None:
    originals = [item for item in copied if "original" in item]
    checked, failures = 0, collections.Counter()
    for item in originals:
        try:
            metadata_equal = fingerprint(item["original"].lstat()) == item["fingerprint"]
            bytes_equal = digest_original(item["original"], item["fingerprint"]) == item["sha256"]
            checked += 1
            if not metadata_equal or not bytes_equal:
                failures["original_source_changed"] += 1
        except OSError:
            failures["original_source_unavailable"] += 1
        except VerificationFailure:
            failures["original_source_changed"] += 1
    report["original_sources"] = {"checked_files": checked, "expected_files": len(originals),
                                  "all_unchanged": not failures if originals else None,
                                  "issue_codes": dict(failures)}
    require(not failures, "original_source_verification_failed")


def verify_mode(options, report: dict) -> None:
    private = None
    try:
        with tempfile.TemporaryDirectory(prefix="pathkeep-agent-history-verify-") as directory:
            private = Path(directory)
            private.chmod(0o700)
            runner = Runner(options.binary, private, report, options.timeout_seconds)
            copied, roots = [], {}
            try:
                if options.synthetic_records is not None:
                    count = options.synthetic_records
                    require(shutil.disk_usage(private).free > count * 6_144 + 512 * 1024 * 1024,
                            "insufficient_disk_budget")
                    root = private / "sources/pi"
                    source = generate_pi(root, count)
                    roots["pi"] = root
                    copied.append({"copied": source, "sha256": digest_file(source), "bytes": source.stat().st_size})
                    report.update({"records": count, "fixture_version": "pi-v3-scale-1"})
                else:
                    selected, report["harnesses"] = select_samples(Path.home())
                    for harness, candidates in selected.items():
                        if not candidates:
                            continue
                        root = private / "sources" / harness
                        for original_root, source in candidates:
                            sample = copy_sample(source, root / source.relative_to(original_root))
                            copied.append(sample)
                            summary = report["harnesses"][harness]
                            summary["copied_files"] = summary.get("copied_files", 0) + 1
                            summary["copied_bytes"] = summary.get("copied_bytes", 0) + sample["bytes"]
                        roots[harness] = root
                    if not roots:
                        report["status"], report["phase"] = "skipped", "no_available_samples"
                        return
                exercise(runner, roots, copied, options.synthetic_records is not None)
                report["status"], report["phase"] = "passed", "complete"
            finally:
                if options.local_samples:
                    previous_phase = report["phase"]
                    report["phase"] = "original_source_verification"
                    verify_originals(copied, report)
                    report["phase"] = previous_phase
                    for summary in report.get("harnesses", {}).values():
                        if summary["selected_files"] and report["status"] == "passed":
                            summary["status"] = "sample_verified"
    finally:
        if private is not None:
            report["temporary_storage_cleanup"] = "completed" if not private.exists() else "failed"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--synthetic-records", type=int)
    mode.add_argument("--local-samples", action="store_true")
    parser.add_argument("--binary", type=Path, default=REPO / "src-tauri/target/debug/pathkeep-agent-history")
    parser.add_argument("--timeout-seconds", type=int, default=3600)
    options = parser.parse_args()
    options.binary = options.binary.absolute()
    synthetic = options.synthetic_records is not None
    report = {"schema_version": 1, "mode": "synthetic" if synthetic else "local_samples",
              "status": "running", "phase": "preflight", "measurements": [],
              "started_at_utc": datetime.now(timezone.utc).isoformat(),
              "platform": {"system": platform.system(), "release": platform.release(),
                           "machine": platform.machine(), "python": platform.python_version()},
              "scope": "streamed Pi fixture" if synthetic else "bounded copied transcripts; not whole-harness coverage",
              "privacy": {"native_content_in_report": False, "native_paths_in_report": False,
                          "native_identifiers_in_report": False, "network_used": False},
              "reproduce": "python3 scripts/agent-history-verify.py " +
                           (f"--synthetic-records {options.synthetic_records}" if synthetic else "--local-samples")}
    if not synthetic:
        report["sampling"] = {"max_files_per_harness": 2, "max_file_bytes": MAX_SAMPLE_BYTES,
                              "minimum_age_seconds": 3600, "custom_roots_tested": False,
                              "desktop_stores_tested": False,
                              "replay_limit": "Repeating selects eligible files again; native identities are deliberately not retained."}
    started = time.perf_counter()
    old_umask = os.umask(0o077)
    exit_code = 0
    try:
        require(not synthetic or options.synthetic_records > 0, "records_must_be_positive")
        require(options.timeout_seconds > 0, "timeout_must_be_positive")
        require(options.binary.is_file(), "built_cli_missing")
        report["build"] = build_fingerprint(options.binary)
        report["runner_sha256"] = digest_file(Path(__file__).absolute())
        verify_mode(options, report)
    except (KeyboardInterrupt, SystemExit):
        report.update({"status": "interrupted", "error_code": "interrupted"})
        exit_code = 130
    except Exception as error:
        if isinstance(error, VerificationFailure):
            code = error.args[0]
        elif isinstance(error, subprocess.TimeoutExpired):
            code = "cli_timeout"
        elif isinstance(error, OSError):
            code = "filesystem_or_process_error"
        else:
            code = "verification_runner_error"
        report.update({"status": "failed", "error_code": code})
        exit_code = 1
    finally:
        report["elapsed_seconds"] = round(time.perf_counter() - started, 6)
        rss = [item["max_rss_bytes"] for item in report["measurements"] if item["max_rss_bytes"] is not None]
        report["peak_command_rss_bytes"] = max(rss) if rss else None
        destination = REPO / "artifacts/agent-history" / ("scale.json" if synthetic else "real-samples.json")
        report["artifact_written"] = True
        pending = None
        try:
            destination.parent.mkdir(parents=True, exist_ok=True)
            with tempfile.NamedTemporaryFile(mode="w", encoding="utf-8", dir=destination.parent,
                                             prefix="aggregate-", suffix=".tmp", delete=False) as output:
                pending = Path(output.name)
                json.dump(report, output, indent=2, ensure_ascii=False)
                output.write("\n")
                output.flush()
                os.fsync(output.fileno())
            pending.replace(destination)
        except OSError:
            report.update({"artifact_written": False, "artifact_error_code": "artifact_write_failed"})
            exit_code = 1
        finally:
            if pending is not None:
                try:
                    pending.unlink(missing_ok=True)
                except OSError:
                    report["artifact_cleanup_error_code"] = "aggregate_temporary_cleanup_failed"
                    exit_code = 1
            os.umask(old_umask)
    print(json.dumps(report, ensure_ascii=False))
    return exit_code


def interrupt(_signum, _frame):
    """Unwind through process and temporary-directory cleanup on termination."""
    raise KeyboardInterrupt


if __name__ == "__main__":
    signal.signal(signal.SIGTERM, interrupt)
    raise SystemExit(main())
