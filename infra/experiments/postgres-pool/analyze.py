"""Publish an allowlisted summary of private proof evidence; never resource IDs."""
import json
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
OUT = ROOT / 'docs/services/postgres-pool-data'


def summarize(filename):
    state = json.loads((ROOT / 'work' / filename).read_text())
    final = state['final']
    submissions = {row['id']: row for row in final['submissions']}
    records = []
    for label, receipt in state['receipts'].items():
        row = submissions[receipt['id']]
        nodes = [n for n in final['nodes'] if n['pipeline_id'] == receipt['id']]
        checks = [n for n in nodes if n['phase'] == 'check' and n['status'] == 'done']
        runs = [n for n in nodes if n['phase'] == 'run']
        record = dict(label=label, language=receipt['fixture']['language'],
                      ordinal=row['ordinal'], terminal=row['terminal'], published=row['published'],
                      verdict=checks[0]['verdict'] if checks else 'DLQ',
                      terminal_seconds=row['seconds'], publication_seconds=row['publication_seconds'],
                      attempts=max((n['attempt'] for n in nodes), default=0),
                      run_cpu_usec=runs[0]['host_cpu_usec'] if runs else None)
        # Burst retries were manual and their delay is not a latency measurement.
        if not label.startswith('burst') and 'client_result_seconds' in receipt:
            record['client_result_seconds'] = receipt['client_result_seconds']
        records.append(record)
    assert all(r['terminal'] and r['published'] for r in records)
    result = dict(accepted=len(records), published=len(records),
                  outcomes=dict(Counter(r['verdict'] for r in records)), cases=records,
                  observations=[e for e in state['events'] if e['event'] != 'burst_retry_completed'])
    return state, result


if __name__ == '__main__':
    OUT.mkdir(exist_ok=True)
    _, first = summarize('pg-pool-first-run-state.json')
    state, second = summarize('pg-pool-run-state.json')
    feed = state['replayed_feed']
    assert [e['cursor'] for e in feed] == list(range(1, len(feed) + 1))
    results = [e for e in feed if e['kind'] == 'result.published']
    assert len({e['submission'] for e in results}) == len(results) == second['accepted']
    assert [e['payload']['ordinal'] for e in results] == list(range(1, len(results) + 1))
    second['event_replay'] = dict(events=len(feed), final_results=len(results),
                                 contiguous_cursors=True, unique_finals=True, ordered_finals=True)
    prepared = json.loads((ROOT / 'work/pg-pool-prepared.json').read_text())
    (OUT / 'summary.json').write_text(json.dumps(dict(environment=prepared, first=first, v2=second), indent=2) + '\n')
    warm_id = next(r['id'] for label, r in state['receipts'].items() if label.startswith('warm'))
    example = [dict(cursor=e['cursor'], kind=e['kind'], payload=e['payload'])
               for e in feed if e['submission'] == warm_id]
    (OUT / 'event-example.json').write_text(json.dumps(example, indent=2) + '\n')
    print(json.dumps(dict(first=first['outcomes'], v2=second['outcomes'], events=second['event_replay'])))
