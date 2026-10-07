#!/usr/bin/env python3
"""Responsibilities: run the existing scale fixture with observable reports and plans on Modal.
Not responsible for: changing product SQL, storage policy, schemas, ciphers or benchmark inputs.
Failure modes: stale executables, lost stage reports, unbounded plan capture, profiler failures
hidden as success, or generated archives surviving the run.
"""
import json
import os
import pathlib
import re
import shutil
import subprocess
import tempfile
import threading
import time
import tomllib


ROOT = pathlib.Path('/state/work')
LOGS = pathlib.Path('/state/area4-observed')
SOURCE = ROOT / 'src-tauri/crates/vault-worker/tests/archive_scale_bench'


def checked(command, name, **kwargs):
    with (LOGS / name).open('w') as log:
        code = subprocess.call(command, stdout=log, stderr=subprocess.STDOUT, **kwargs)
    print(f'{name}: exit {code}', flush=True)
    if code:
        print((LOGS / name).read_text()[-14000:], flush=True)
        raise SystemExit(code)


def instrument_harness():
    """Adapt a copy of the benchmark, leaving its production dependencies untouched."""
    source = (SOURCE / 'turso_intelligence.rs').read_text()
    before = '    vault_core::run_core_intelligence_with_progress(\n'
    assert source.count(before) == 1
    source = source.replace(before, '''    let mut captured_phase = String::new();
    let mut capturing = false;
    let report = vault_core::run_core_intelligence_with_progress(
''')
    before = '        |p| {\n'
    assert source.count(before) == 1
    source = source.replace(before, '''        |p| {
            if capturing {
                save_plans(vault, &captured_phase);
                capturing = false;
            }
            if captured_phase != p.phase {
                captured_phase = p.phase.clone();
                for db in [Db::Archive, Db::Intelligence] {
                    vault.store().capture_query_plans(db, true).unwrap();
                }
                capturing = true;
            }
''')
    before = '    let completed = Instant::now();\n'
    assert source.count(before) == 1
    source = source.replace(before, '''    let completed = Instant::now();
    save_plans(vault, &captured_phase);
    let report_path = std::path::Path::new(&std::env::var("PATHKEEP_INTELLIGENCE_PROFILE_DIR").unwrap())
        .join(if full { "full-report.json" } else { "incremental-report.json" });
    std::fs::write(report_path, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
    println!("INTELLIGENCE_REPORT {}", serde_json::to_string(&report.stage_timings_ms).unwrap());
''')
    source = source.replace('env!("CARGO_MANIFEST_DIR")',
                            json.dumps(str(ROOT/'src-tauri/crates/vault-worker')))
    source = source.replace('start.elapsed().as_millis(),\n        super::turso_reads::peak_rss_bytes()',
                            'completed.duration_since(start).as_millis(),\n        super::turso_reads::peak_rss_bytes()')
    before = 'fn read(vault: &Vault, pass: &str, operation: &str) {\n'
    assert source.count(before) == 1
    source = source.replace(before, '''fn save_plans(vault: &Vault, phase: &str) {
    use std::io::Write;
    for db in [Db::Archive, Db::Intelligence] {
        vault.store().capture_query_plans(db, false).unwrap();
        let plans = vault.store().take_query_plans(db).unwrap();
        let path = std::path::Path::new(&std::env::var("PATHKEEP_INTELLIGENCE_PROFILE_DIR").unwrap()).join("plans.jsonl");
        let mut file = std::fs::OpenOptions::new().create(true).append(true).open(path).unwrap();
        for plan in plans {
            writeln!(file, "{}", serde_json::json!({"phase":phase,"db":format!("{db:?}"),"sql":plan.sql,"steps":plan.steps})).unwrap();
        }
    }
}
fn read(vault: &Vault, pass: &str, operation: &str) {
    if pass == "cold" {
        for db in [Db::Archive, Db::Intelligence] {
            vault.store().capture_query_plans(db, true).unwrap();
        }
    }
''')
    # Keep read timings free of EXPLAIN work: plans come from one additional, untimed read.
    source = source.replace('''                read(&vault, "warm", operation);
''', '''                read(&vault, "warm", operation);
                for db in [Db::Archive, Db::Intelligence] {
                    vault.store().capture_query_plans(db, true).unwrap();
                }
                read(&vault, "plan-probe", operation);
                save_plans(&vault, operation);
''')
    source = source.replace('''    if pass == "cold" {
        for db in [Db::Archive, Db::Intelligence] {
            vault.store().capture_query_plans(db, true).unwrap();
        }
    }
''', '')
    before = '    child(root.path(), "incremental");\n'
    assert source.count(before) == 1
    source = source.replace(before, before + '''    let vault = super::turso_reads::open(root.path());
    explain_writes(&vault);
    vault.close().unwrap();
    drop(vault);
''')
    cases = []
    for name in ['build_rows.rs', 'intelligence_structural_stream.rs']:
        file = ROOT / 'src-tauri/crates/vault-core/src/intelligence' / name
        for match in re.finditer(r'\.prepare\(\s*"([^"]+)"', file.read_text()):
            sql = match.group(1)
            if any(table in sql for table in ['visit_derived_facts_rows', 'visit_assignments', 'session_domains']):
                count = max(map(int,re.findall(r'\?(\d+)',sql)))
                cases.append((name,sql,count))
    source += '''\nfn explain_writes(vault: &Vault) {
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new().create(true).append(true)
        .open(std::path::Path::new(&std::env::var("PATHKEEP_INTELLIGENCE_PROFILE_DIR").unwrap()).join("plans.jsonl")).unwrap();
'''
    for name,sql,count in cases:
        source += '''    {
        let sql = ''' + json.dumps(sql) + ''';
        let steps = vault.store().write(Db::Intelligence,Priority::Batch,move |s| {
            s.query(&format!("EXPLAIN QUERY PLAN {sql}"),vec![vault_store::Value::Integer(1);''' + str(count) + '''],|r|Ok(r.get::<String>(3)?))
        }).unwrap();
        writeln!(file,"{}",serde_json::json!({"phase":"write-plan-probe","source":''' + json.dumps(name) + ''',"sql":sql,"steps":steps})).unwrap();
    }
'''
    source += '}\n'
    source += '''
#[test]
fn observed_small_rebuild() {
    let root = tempfile::tempdir().unwrap();
    let vault = super::smoke_reads::open(root.path());
    super::smoke_reads::seed(&vault);
    rebuild(&vault, true);
    explain_writes(&vault);
    vault.close().unwrap();
}
'''
    path = LOGS / 'turso_intelligence.rs'
    path.write_text(source)
    wrapper = LOGS / 'area4_scale.rs'
    small = (SOURCE/'turso_reads.rs').read_text()
    small = small.replace('const VISITS: i64 = 14_400_000;', 'const VISITS: i64 = 10_000;')
    small = small.replace('const PAGES: i64 = 2_880_000;', 'const PAGES: i64 = 2_000;')
    small = small.replace('2880000', '2000')
    (LOGS/'smoke_reads.rs').write_text(small)
    wrapper.write_text('''//! Responsibilities: retain the existing fixture and expose its returned report.
//! Not responsible for product behavior. Failure modes are checked by the original harness.
static COUNT_HEAP: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
#[allow(dead_code, unused_imports)]
#[path = ''' + json.dumps(str(SOURCE / 'turso_reads.rs')) + ''']
mod turso_reads;
#[allow(dead_code, unused_imports)]
#[path = "smoke_reads.rs"]
mod smoke_reads;
#[path = "turso_intelligence.rs"]
mod turso_intelligence;
''')
    return wrapper


def toml_value(value):
    if isinstance(value, dict):
        return '{'+', '.join(f'{json.dumps(k)} = {toml_value(v)}' for k,v in value.items())+'}'
    return json.dumps(value)


def build():
    """Let Cargo select artifacts and LTO inputs, using the worker's dependency graph/profile."""
    env = {**os.environ, 'CARGO_TARGET_DIR':str(ROOT / 'src-tauri/target'),
           'CARGO_PROFILE_RELEASE_DEBUG':'1', 'CARGO_PROFILE_RELEASE_STRIP':'none',
           'CARGO_BUILD_JOBS':'4'}
    workspace = tomllib.loads((ROOT/'src-tauri/Cargo.toml').read_text())
    worker_root = ROOT/'src-tauri/crates/vault-worker'
    worker = tomllib.loads((worker_root/'Cargo.toml').read_text())
    dependencies = {**worker['dependencies'], **worker['dev-dependencies']}
    for name, value in dependencies.items():
        if isinstance(value, dict):
            value = dict(value)
            if value.pop('workspace', False):
                inherited = workspace['workspace']['dependencies'][name]
                inherited = {'version':inherited} if isinstance(inherited,str) else dict(inherited)
                value = {**inherited, **value}
            if 'path' in value:
                value['path'] = str((worker_root/value['path']).resolve())
        dependencies[name] = value
    manifest = LOGS/'cargo/Cargo.toml'
    manifest.parent.mkdir(exist_ok=True)
    manifest.write_text('''[package]
name = "area4-measurement"
version = "0.0.0"
edition = "2024"
[workspace]
resolver = "2"
[[test]]
name = "area4_scale"
path = '''+json.dumps(str(instrument_harness()))+'\n[dependencies]\n'+
        '\n'.join(f'{name} = {toml_value(value)}' for name,value in dependencies.items())+
        '\n[profile.release]\n'+
        '\n'.join(f'{name} = {toml_value(value)}' for name,value in workspace['profile']['release'].items())+'\n')
    # Existing versions remain pinned; Cargo only adds this measurement package to the copy.
    shutil.copy2(ROOT/'src-tauri/Cargo.lock',manifest.with_name('Cargo.lock'))
    with (LOGS / 'build.jsonl').open('w') as output, (LOGS / 'build.log').open('w') as log:
        code = subprocess.call(['cargo','test','--offline','--release','--manifest-path',str(manifest),
                                '--test','area4_scale','--no-run','--message-format=json'],
                               cwd=ROOT/'src-tauri', env=env, stdout=output, stderr=log)
    print('benchmark build exit', code, flush=True)
    if code:
        print((LOGS/'build.log').read_text()[-14000:], flush=True)
        raise SystemExit(code)
    artifacts = [json.loads(line) for line in (LOGS/'build.jsonl').read_text().splitlines()]
    return next(pathlib.Path(item['executable']) for item in reversed(artifacts)
                if item.get('reason')=='compiler-artifact' and item.get('executable')
                and item['target']['name']=='area4_scale')


def main():
    LOGS.mkdir(exist_ok=True)
    # Retain cached E2E output remotely, so offload does not copy it outside the allowed local paths.
    artifacts = ROOT/'artifacts/e2e'
    if artifacts.exists():
        artifacts.rename(LOGS/f'cached-e2e-{time.time_ns()}')
    executable = build()
    checked([str(executable),'--exact',
             'turso_intelligence::skip_baseline_rebuild_requires_existing_intelligence_state',
             '--nocapture'], 'harness-check.log', cwd=ROOT)
    profiles = LOGS/'profile'
    profiles.mkdir(exist_ok=True)
    fixture = pathlib.Path(tempfile.mkdtemp(prefix='pathkeep-area4-', dir='/tmp'))
    env = {**os.environ, 'PATHKEEP_TEST_BLOCK_NETWORK':'1', 'DEBUGINFOD_URLS':'',
           'PATHKEEP_INTELLIGENCE_BENCH_ROOT':str(fixture),
           'PATHKEEP_INTELLIGENCE_PROFILE_DIR':str(profiles),
           'PATHKEEP_PROFILE_STAGES':'1', 'PATHKEEP_STAGE_TRACE':str(profiles/'stages.jsonl')}
    smoke = LOGS/'smoke'
    smoke.mkdir(exist_ok=True)
    smoke_env = {**env, 'PATHKEEP_INTELLIGENCE_PROFILE_DIR':str(smoke),
                 'PATHKEEP_STAGE_TRACE':str(smoke/'stages.jsonl')}
    checked(['python3',str(ROOT/'scripts/profile-native.py'),'--output',str(smoke/'profile.json'),
             '--interval','5','--',str(executable),'--exact',
             'turso_intelligence::observed_small_rebuild','--nocapture'],
            'smoke.log',env=smoke_env,cwd=ROOT)
    smoke_report = json.loads((smoke/'full-report.json').read_text())
    assert smoke_report['stageTimingsMs'], 'missing returned stage timings'
    records = [json.loads(line) for line in (smoke/'stages.jsonl').read_text().splitlines()]
    stages = {row['stage'] for row in records if row['type']=='stage'}
    required = {'facts-begin','facts-end','daily-begin','daily-end','navigation-begin',
                'aggregates-begin','structural-end','publish-begin','publish-end'}
    assert required <= stages, ('missing stage probes',required-stages)
    assert (smoke/'plans.jsonl').stat().st_size > 0, 'missing native query plans'
    print('SMOKE stage reports, stage boundaries and native query plans verified',flush=True)
    print('ENV', os.uname(), flush=True)
    print(pathlib.Path('/proc/meminfo').read_text().splitlines()[0], flush=True)
    print('affinity', sorted(os.sched_getaffinity(0)), flush=True)
    print('fixture',fixture,flush=True)
    stop = threading.Event()

    def monitor():
        last = ''
        with (LOGS/'resources.log').open('w',buffering=1) as log:
            while not stop.is_set():
                log.write(time.strftime('%Y-%m-%dT%H:%M:%SZ',time.gmtime())+'\n')
                log.write(pathlib.Path('/proc/meminfo').read_text())
                log.write(subprocess.check_output(['ps','-eo','pid,ppid,rss,comm','--sort=-rss'],text=True))
                path = LOGS/'scale.log'
                if path.exists():
                    lines = [line for line in path.read_text(errors='replace').splitlines() if 'BENCH ' in line]
                    if lines and lines[-1] != last:
                        last = lines[-1]
                        print('PROGRESS',last,flush=True)
                stop.wait(60)

    thread = threading.Thread(target=monitor)
    thread.start()
    try:
        checked([str(executable),'--exact','turso_intelligence::encrypted_intelligence_at_scale',
                 '--ignored','--nocapture'], 'scale.log', env=env, cwd=ROOT)
        for line in (LOGS/'scale.log').read_text().splitlines():
            if 'INTELLIGENCE_REPORT' in line or ('INTELLIGENCE_BENCH' in line and 'phase=' not in line):
                print(line,flush=True)
    finally:
        stop.set()
        thread.join()
        shutil.rmtree(fixture,ignore_errors=True)
        print('CLEANUP data_removed',not fixture.exists(),flush=True)


if __name__ == '__main__':
    main()
