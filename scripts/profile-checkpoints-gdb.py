"""Responsibilities: time checkpoint entry/return in an unchanged native executable.
Not responsible for: selecting checkpoint policy, changing ciphers or estimating CPU time.
Failure modes: missed returns, wrong function boundaries, unreported debugger overhead,
or incomplete logs presented as a completed profile. Run inside GDB, via profile-native.py.
"""
import json
import os
import time

import gdb


output = open(os.environ["PATHKEEP_CHECKPOINT_TRACE"], "w", buffering=1)
pending = {}
sequence = 0


class Returned(gdb.FinishBreakpoint):
    def __init__(self, record):
        super().__init__(gdb.newest_frame(), internal=True)
        self.record = record
        self.silent = True

    def stop(self):
        ended = time.monotonic_ns()
        self.record["finished_ns"] = ended
        self.record["elapsed_ns"] = ended - self.record["started_ns"]
        pending.pop(self.record["sequence"])
        output.write(json.dumps(self.record) + "\n")
        return False

    def out_of_scope(self):
        self.record["error"] = "checkpoint frame exited without a return breakpoint"
        pending.pop(self.record["sequence"], None)
        output.write(json.dumps(self.record) + "\n")


class Entered(gdb.Breakpoint):
    def stop(self):
        global sequence
        entered = time.monotonic_ns()
        sequence += 1
        thread = gdb.selected_thread()
        caller = gdb.newest_frame().older().find_sal()
        # Optimized Rust omits argument DWARF. Measure both modes together without
        # guessing registers or relying on a private Rust calling convention.
        record = {"sequence": sequence, "tid": thread.ptid[1], "thread": thread.name,
                  "function": gdb.newest_frame().name(), "mode": "all",
                  "caller_file": caller.symtab.filename, "caller_line": caller.line}
        Returned(record)
        pending[sequence] = record
        record["started_ns"] = time.monotonic_ns()
        record["entry_handler_ns"] = record["started_ns"] - entered
        return False


def finished(event):
    for record in pending.values():
        output.write(json.dumps({**record, "error": "process exited inside checkpoint"}) + "\n")
    output.close()


Entered(os.environ.get("PATHKEEP_CHECKPOINT_SYMBOL", "vault_store::checkpoint::run"), internal=True)
gdb.events.exited.connect(finished)
