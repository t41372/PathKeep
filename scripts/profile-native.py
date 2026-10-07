#!/usr/bin/env python3
"""Responsibilities: sample a native process with GDB when perf events are unavailable.
Not responsible for: compiling the program, changing its inputs, or estimating CPU time.
Failure modes: debugger errors masquerade as success, idle stacks dominate the report,
unbounded trace retention changes memory use, or sampling pauses are omitted from the report.
"""
import argparse
import ast
import collections
import json
import os
import pathlib
import queue
import re
import subprocess
import threading
import time


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", required=True)
    parser.add_argument("--interval", type=float, default=5.0)
    parser.add_argument("program", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    program = args.program[1:] if args.program[:1] == ["--"] else args.program
    output = pathlib.Path(args.output)
    output.parent.mkdir(parents=True, exist_ok=True)
    debugger = subprocess.Popen(
        ["gdb", "--quiet", "--nx", "--interpreter=mi2", "--args", *program],
        stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
        text=True, bufsize=1,
    )
    lines = queue.Queue()

    def read_lines():
        for line in debugger.stdout:
            lines.put(line.rstrip())
        lines.put(None)

    threading.Thread(target=read_lines, daemon=True).start()
    token = 0
    phase = "startup"
    pid = None
    exited = None
    stopped = True
    console = []
    sample_count = 0
    counts = collections.Counter()
    waits = ("futex", "epoll_wait", "nanosleep", "pthread_cond", "__GI_poll", "ppoll")
    pauses = 0.0
    phase_io = {}
    trace_log = output.with_suffix(".samples.jsonl").open("w")
    event_log = output.with_suffix(".events.jsonl").open("w")
    inferior_log = output.with_suffix(".inferior.log").open("w", buffering=1)

    def io_counters():
        if pid is None:
            return {}
        try:
            return {k: int(v) for k, v in
                    (line.split(":", 1) for line in pathlib.Path(f"/proc/{pid}/io").read_text().splitlines())}
        except FileNotFoundError:
            return {}

    def receive(timeout=None):
        nonlocal phase, pid, exited, stopped
        line = lines.get(timeout=timeout)
        if line is None:
            if exited is None:
                raise RuntimeError("GDB exited before reporting the inferior's result")
            return ""
        if line.startswith("=thread-group-started"):
            pid = int(re.search(r'pid="(\d+)"', line).group(1))
        if line.startswith("*running"):
            stopped = False
        if line.startswith("*stopped"):
            stopped = True
            if 'reason="exited-normally"' in line:
                exited = 0
            elif 'reason="exited"' in line:
                value = re.search(r'exit-code="([^\"]+)"', line)
                exited = int(value.group(1), 8) if value else 1
        text = line
        if line[:1] in ("~", "@", "&"):
            try:
                text = ast.literal_eval(line[1:])
            except (SyntaxError, ValueError):
                text = line
            console.append(text)
        if "Python Exception" in text:
            raise RuntimeError(text)
        benchmark_line = any(marker in text for marker in
                             ("INTELLIGENCE_BENCH", "INTELLIGENCE_REPORT", "test result:", "panicked at"))
        native_line = line.startswith("@") or (line and
            not line.startswith(("=", "*", "~", "&", "(gdb)")) and not re.match(r"^\d+\^", line))
        if native_line:
            inferior_log.write(text if text.endswith("\n") else text+"\n")
        if benchmark_line or native_line:
            print(text, end="" if text.endswith("\n") else "\n", flush=True)
        if benchmark_line:
            event_log.write(json.dumps({"time_ns": time.monotonic_ns(), "text": text}) + "\n")
            event_log.flush()
            marker = re.search(r"phase=([a-z-]+)", text)
            if marker and marker.group(1) != phase:
                phase = marker.group(1)
                phase_io[phase] = io_counters()
        return line

    def command(body):
        nonlocal token
        token += 1
        debugger.stdin.write(f"{token}{body}\n")
        debugger.stdin.flush()
        while True:
            line = receive(60)
            if line.startswith(f"{token}^error"):
                raise RuntimeError(line)
            if line.startswith(f"{token}^"):
                return line

    try:
        command("-gdb-set mi-async on")
        command("-gdb-set pagination off")
        command("-gdb-set confirm off")
        command("-gdb-set debuginfod enabled off")
        command("-gdb-set print frame-arguments none")
        command('-interpreter-exec console "handle SIGPIPE nostop noprint pass"')
        if os.environ.get("PATHKEEP_PROFILE_CHECKPOINTS") == "1":
            helper = pathlib.Path(__file__).with_name("profile-checkpoints-gdb.py")
            command('-interpreter-exec console ' + json.dumps(f"source {helper}"))
        if os.environ.get("PATHKEEP_PROFILE_STAGES") == "1":
            helper = pathlib.Path(__file__).with_name("profile-stages-gdb.py")
            command('-interpreter-exec console ' + json.dumps(f"source {helper}"))
        command("-exec-run")
        next_sample = time.monotonic() + args.interval
        while exited is None:
            try:
                line = receive(max(0.001, next_sample - time.monotonic()))
                if line.startswith("*stopped") and exited is None:
                    command("-exec-continue")
            except queue.Empty:
                began_ns = time.monotonic_ns()
                began = time.monotonic()
                command("-exec-interrupt --all")
                while exited is None and not stopped:
                    if receive(60).startswith("*stopped"):
                        break
                if exited is not None:
                    break
                console.clear()
                command('-interpreter-exec console "thread apply all bt 48"')
                trace = "".join(console)
                counters = io_counters()
                command("-exec-continue")
                pause = time.monotonic() - began
                pauses += pause
                sample = {"phase": phase, "pause_seconds": pause,
                    "started_ns": began_ns, "finished_ns": time.monotonic_ns(),
                    "io": counters, "trace": trace}
                sample_count += 1
                for stack in re.split(r"(?m)^Thread ", trace):
                    leaf = re.search(r"(?m)^#0\s+(?:0x[0-9a-f]+\s+in\s+)?(.+)", stack)
                    if leaf and not any(word in stack for word in waits):
                        counts[(phase, leaf.group(1))] += 1
                trace_log.write(json.dumps(sample) + "\n")
                trace_log.flush()
                next_sample = time.monotonic() + args.interval
        command("-gdb-exit")
    finally:
        if debugger.poll() is None:
            debugger.terminate()
        debugger.wait(timeout=30)
        trace_log.close()
        event_log.close()
        inferior_log.close()
        output.write_text(json.dumps({"program": program, "exit": exited,
            "sampling_pause_seconds": pauses, "phase_io": phase_io,
            "sample_count": sample_count, "samples_file": str(output.with_suffix(".samples.jsonl"))}, indent=2))
    print("PROFILE", json.dumps({"samples": sample_count, "pause_seconds": pauses,
        "top_nonwait_leaves": [(phase, leaf, count) for (phase, leaf), count in counts.most_common(24)]}), flush=True)
    return exited if exited is not None else 1


if __name__ == "__main__":
    raise SystemExit(main())
