#!/usr/bin/env python3
"""Responsibilities: reduce completed area-4 measurements, stage boundaries and SQL evidence.
Not responsible for: running the workload, changing SQL, or treating nested batch times as additive.
Failure modes: partial runs accepted, duplicated stage markers, nested durations double-counted,
or cache-warming probes presented as cold/warm measurements.
"""
import argparse
import collections
import json
import pathlib
import re
import time


def json_lines(path):
    with path.open() as file:
        for line in file:
            yield json.loads(line)


def stage_times(report, records):
    marks = collections.defaultdict(list)
    for row in records:
        if row['type'] == 'stage':
            marks[row['stage']].append(row['time_ns'])
    def interval(begin, end):
        assert len(marks[begin]) == len(marks[end]) == 1, (begin,end,marks)
        return (marks[end][0]-marks[begin][0])/1e9
    timed = report['stageTimingsMs']
    result = {
        'visit_derived_facts_s':timed['visitDeriveMs']/1000,
        'daily_s':timed['dailyRollupMs']/1000,
        'navigation_s':interval('navigation-begin','aggregates-begin'),
        'aggregates_s':interval('aggregates-begin','structural-end'),
        'publish_s':interval('publish-begin','publish-end'),
        'reported_structural_s':timed['structuralRebuildMs']/1000,
        'reported_total_before_overview_s':timed['totalMs']/1000,
    }
    result['structural_probe_difference_s'] = (
        result['navigation_s']+result['aggregates_s']-result['reported_structural_s'])
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('directory',type=pathlib.Path)
    args = parser.parse_args()
    root = args.directory
    artifacts = pathlib.Path('/state/work/artifacts/e2e')
    if artifacts.exists():
        artifacts.rename(root/f'cached-e2e-{time.time_ns()}')
    profile = root/'profile'
    metadata = json.loads((profile/'full-rebuild.json').read_text())
    assert metadata['exit'] == 0, 'profiled rebuild did not finish successfully'
    full = json.loads((profile/'full-report.json').read_text())
    incremental = json.loads((profile/'incremental-report.json').read_text())
    assert full['visitDerivedFacts'] == 14_400_000, full
    assert incremental['visitDerivedFacts'] == 10_000, incremental
    records = list(json_lines(profile/'stages.jsonl'))
    stages = stage_times(full, records)
    log = (root/'scale.log').read_text()
    assert 'data_removed=true' in log, 'fixture cleanup was not reported'
    measurements = {}
    for label,elapsed,rss in re.findall(
            r'INTELLIGENCE_BENCH (full|incremental-10000) elapsed_ms=(\d+) peak_rss_bytes=Some\((\d+)\)', log):
        measurements[label] = {'seconds':int(elapsed)/1000,'peak_rss_bytes':int(rss)}
    for temperature,operation,elapsed,rss in re.findall(
            r'INTELLIGENCE_BENCH (cold|warm) (overview|domain|sessions) elapsed_ms=([\d.]+) peak_rss_bytes=Some\((\d+)\)', log):
        measurements[f'{operation}_{temperature}'] = {'milliseconds':float(elapsed),'peak_rss_bytes':int(rss)}
    assert len(measurements) == 8, measurements
    writes = re.findall(r'INTELLIGENCE_BENCH interactive_write_wait_ms=([\d.]+)', log)
    measurements['interactive_star_ms'] = float(writes[0])
    stages['other_work_and_overview_s'] = measurements['full']['seconds']-sum(
        stages[k] for k in ['visit_derived_facts_s','daily_s','navigation_s','aggregates_s','publish_s'])
    batches = collections.defaultdict(list)
    unmatched = collections.Counter()
    for row in records:
        if row['type'] == 'batch':
            batches[row['function']].append(row['elapsed_ns']/1e9)
        elif row['type'] == 'unmatched':
            unmatched[row['function']] += 1
    summaries = []
    for function,times in batches.items():
        times.sort()
        summaries.append({'function':function,'calls':len(times),'inclusive_seconds':sum(times),
                          'unmatched_returns':unmatched[function],
                          'median_ms':times[len(times)//2]*1000,
                          'p95_ms':times[int((len(times)-1)*.95)]*1000,'max_ms':times[-1]*1000})
    summaries.sort(key=lambda row:row['inclusive_seconds'],reverse=True)
    plans = {}
    for row in json_lines(profile/'plans.jsonl'):
        key = (re.sub(r'\s+',' ',row['sql']).strip(),tuple(row['steps']))
        if key not in plans:
            plans[key] = {**row,'phases':[]}
        if row['phase'] not in plans[key]['phases']:
            plans[key]['phases'].append(row['phase'])
    result = {'measurements':measurements,'stages':stages,
              'batch_functions_inclusive':summaries,
              'probe_limitations':{
                  'unmatched_returns':dict(unmatched),
                  'unavailable':[r for r in records if r['type']=='unavailable'],
                  'note':'Batch totals cover matched returns only and include nested work; they are not additive stage totals.'},
              'sampling_pause_seconds':metadata['sampling_pause_seconds'],
              'sample_count':metadata['sample_count'],
              'loads':re.findall(r'INTELLIGENCE_BENCH (.*load average:.*)',log),
              'unique_plans':list(plans.values()),
              'incremental_stage_report':incremental['stageTimingsMs']}
    (root/'summary.json').write_text(json.dumps(result,indent=2))
    print(json.dumps({k:v for k,v in result.items() if k!='unique_plans'},indent=2))
    print('NATIVE_QUERY_PLANS',len(plans))


if __name__ == '__main__':
    main()
