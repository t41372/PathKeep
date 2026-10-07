"""Responsibilities: record existing stage boundaries and SQL batch-function durations in GDB.
Not responsible for: changing production functions or attributing nested times additively.
Failure modes: optimized-away probes, duplicated source locations, missing returns, or sampling
pauses mistaken for application execution. Unavailable batch probes are recorded explicitly.
"""
import json
import os
import pathlib
import time

import gdb


output = open(os.environ['PATHKEEP_STAGE_TRACE'], 'w', buffering=1)
source = pathlib.Path('/state/work/src-tauri/crates/vault-core/src/intelligence/intelligence_rebuild.rs')
lines = source.read_text().splitlines()


def record(value):
    output.write(json.dumps({'time_ns':time.monotonic_ns(), **value})+'\n')


class Boundary(gdb.Breakpoint):
    def __init__(self, stage, text):
        matches = [i+1 for i,line in enumerate(lines) if text in line]
        assert len(matches) == 1, (stage,matches)
        super().__init__(f'{source}:{matches[0]}', internal=True)
        self.stage = stage
        self.silent = True

    def stop(self):
        record({'type':'stage','stage':self.stage,'function':gdb.newest_frame().name()})
        return False


class Returned(gdb.FinishBreakpoint):
    def __init__(self, name):
        super().__init__(gdb.newest_frame(), internal=True)
        self.name = name
        self.silent = True
        self.started = time.monotonic_ns()

    def stop(self):
        record({'type':'batch','function':self.name,'started_ns':self.started,
                'elapsed_ns':time.monotonic_ns()-self.started})
        return False

    def out_of_scope(self):
        record({'type':'unmatched','function':self.name,'started_ns':self.started})


class Batch(gdb.Breakpoint):
    def __init__(self, name):
        super().__init__(name, internal=True)
        self.name = name
        self.silent = True

    def stop(self):
        if gdb.newest_frame().type() == gdb.INLINE_FRAME:
            record({'type':'unavailable','function':self.name,'reason':'inlined frame'})
            self.enabled = False
        else:
            Returned(self.name)
        return False


for stage,text in [
    ('facts-begin','let facts_started ='),
    ('facts-end','timings.visit_derive_ms +='),
    ('daily-begin','let daily_started ='),
    ('daily-end','timings.daily_rollup_ms +='),
    ('navigation-begin','build_stages::navigation(vault,'),
    ('aggregates-begin','build_stages::aggregates(vault,'),
    ('structural-end','timings.structural_rebuild_ms +='),
    ('publish-begin','build_pipeline::publish(vault,'),
    ('publish-end','build_state::collect_old_generations(vault,'),
    ('overview-begin','super::intelligence_overview::warm_overview_snapshots(vault,'),
]:
    Boundary(stage,text)

for name in ['build_rows::source_page','build_rows::derived_page','build_rows::persist_facts',
             'build_refind::page','build_state::save']:
    name = 'vault_core::intelligence::'+name
    try:
        Batch(name)
    except gdb.error as error:
        record({'type':'unavailable','function':name,'reason':str(error)})

gdb.events.exited.connect(lambda event: output.close())
