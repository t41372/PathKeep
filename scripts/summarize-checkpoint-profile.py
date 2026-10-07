#!/usr/bin/env python3
"""Responsibilities: summarize checkpoint intervals and stack samples inside the Sandbox.
Not responsible for: running benchmarks, tuning storage or inferring CPU time from GDB samples.
Failure modes: overlapping writers double-count wall time, sampler pauses inflate checkpoint
time, incomplete runs look complete, or idle futex frames hide active encryption stacks.
"""
import argparse
import collections
import json
import pathlib
import re


def merged(intervals):
    result = []
    for start, end in sorted(intervals):
        if result and start <= result[-1][1]:
            result[-1] = (result[-1][0], max(result[-1][1], end))
        else:
            result.append((start, end))
    return result


def duration(intervals):
    return sum(end - start for start, end in merged(intervals)) / 1e9


def intersection(left, right):
    left, right = merged(left), merged(right)
    result = []
    i = j = 0
    while i < len(left) and j < len(right):
        a, b = left[i]
        c, d = right[j]
        if max(a, c) < min(b, d):
            result.append((max(a, c), min(b, d)))
        if b < d:
            i += 1
        else:
            j += 1
    return result


def records(path):
    with path.open() as file:
        for line in file:
            yield json.loads(line)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=pathlib.Path)
    args = parser.parse_args()
    directory = args.directory
    metadata = json.loads((directory / "full-rebuild.json").read_text())
    if metadata["exit"] != 0:
        raise RuntimeError("profiled rebuild did not complete successfully")
    events = list(records(directory / "full-rebuild.events.jsonl"))
    completed = next(e for e in events if "INTELLIGENCE_BENCH full elapsed_ms=" in e["text"])
    elapsed_ms = int(re.search(r"full elapsed_ms=(\d+)", completed["text"])[1])
    # Output receipt can lag the benchmark clock slightly; retain that boundary definition.
    end = completed["time_ns"]
    window = [(end - elapsed_ms * 1_000_000, end)]
    checkpoints = list(records(directory / "checkpoints.jsonl"))
    errors = [r for r in checkpoints if "error" in r]
    intervals = collections.defaultdict(list)
    by_caller = collections.defaultdict(list)
    by_writer_and_caller = collections.defaultdict(list)
    counts = collections.Counter()
    caller_counts = collections.Counter()
    for row in checkpoints:
        if "finished_ns" not in row:
            continue
        clipped = intersection([(row["started_ns"], row["finished_ns"])], window)
        if clipped:
            intervals[row["thread"]].extend(clipped)
            counts[row["thread"]] += 1
            caller = f'{row.get("caller_file", "unrecorded")}:{row.get("caller_line", 0)}'
            by_caller[caller].extend(clipped)
            by_writer_and_caller[f'{row["thread"]}/{caller}'].extend(clipped)
            caller_counts[caller] += 1
    samples = records(directory / "full-rebuild.samples.jsonl")
    pauses = []
    stack_counts = collections.Counter()
    phase_counts = collections.Counter()
    leaves = collections.Counter()
    for sample in samples:
        pauses.extend(intersection([(sample["started_ns"], sample["finished_ns"])], window))
        phase_counts[sample["phase"]] += 1
        for stack in re.split(r"(?m)^Thread ", sample["trace"]):
            frames = re.findall(r"(?m)^#\d+.*", stack)
            if not frames:
                continue
            # Waiting predicates apply to leaf frames, not a futex namespace elsewhere.
            if any(wait in "\n".join(frames[:3]) for wait in
                   ("futex_wait", "__futex_abstimed_wait", "epoll_wait", "nanosleep",
                    "pthread_cond_wait", "__GI_poll", "ppoll")):
                continue
            phase = sample["phase"]
            stack_counts[(phase, "active")] += 1
            for name, needle in [("software_aes", "softaes::"),
                                 ("explicit_checkpoint", "vault_store::checkpoint::run"),
                                 ("wal_checkpoint", "WalFile::checkpoint_inner")]:
                if needle in stack:
                    stack_counts[(phase, name)] += 1
            leaves[frames[0]] += 1
    all_intervals = merged([pair for rows in intervals.values() for pair in rows])
    result = {
        "rebuild_wall_seconds": elapsed_ms / 1000,
        "boundary": "full-result output receipt minus benchmark elapsed_ms",
        "checkpoint_calls_in_window": dict(counts),
        "checkpoint_seconds_by_thread": {k: duration(v) for k, v in intervals.items()},
        "checkpoint_calls_by_callsite": dict(caller_counts),
        "checkpoint_seconds_by_callsite": {k: duration(v) for k, v in by_caller.items()},
        "checkpoint_seconds_by_writer_and_callsite": {
            k: duration(v) for k, v in by_writer_and_caller.items()},
        "checkpoint_wall_union_seconds": duration(all_intervals),
        "sampling_pause_seconds_in_window": duration(pauses),
        "sampling_pause_inside_checkpoints_seconds": duration(intersection(all_intervals, pauses)),
        "entry_handler_seconds_all_calls": sum(r.get("entry_handler_ns", 0) for r in checkpoints) / 1e9,
        "unfinished_checkpoint_records": errors,
        "samples_by_phase": dict(phase_counts),
        "active_stack_observations": [{"phase": p, "kind": k, "count": v}
                                      for (p, k), v in sorted(stack_counts.items())],
        "top_active_leaves": leaves.most_common(16),
    }
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
