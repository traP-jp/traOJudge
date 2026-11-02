"""Check whether short-case timer quantization explains the platform comparison."""
import argparse
from concurrent.futures import ThreadPoolExecutor
import json
from pathlib import Path
import statistics

from run import Managed, Queued


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--target', choices=['microvm', 'ec2'], required=True)
    p.add_argument('--runtime', default='runsc', choices=['runsc', 'runc'])
    a = p.parse_args()
    root = Path(__file__).resolve().parents[3]
    file = 'fairness-outputs.json' if a.target == 'microvm' else 'fairness-ec2-outputs.json'
    out = next(iter(json.loads((root / 'work' / file).read_text()).values()))
    clients = [Managed(out, i) for i in range(3)] if a.target == 'microvm' else [Queued(out, i) for i in range(2)]
    for client in clients:
        if isinstance(client, Queued):
            client.runtime = a.runtime
    report = {'target': a.target, 'runtime': a.runtime if a.target == 'ec2' else 'microvm-native',
              'samples_per_batch': 10, 'measurements': []}
    suffix = 'native' if a.target == 'microvm' else a.runtime
    target = root / f'work/fairness-long-{a.target}-{suffix}.json'

    def save():
        target.write_text(json.dumps(report, indent=2) + '\n')

    def measure(client, phase, workload, body):
        return {'client': client.label, 'phase': phase, 'workload': workload, 'result': client.run(body)}

    for name, mode, count in [('long_cpu', 1, 300_000_000), ('long_memory', 2, 16_000_000)]:
        body = {'mode': mode, 'iterations': count, 'samples': 10}
        for c in clients:
            report['measurements'].append(measure(c, 'sequential', name, body)); save()
        with ThreadPoolExecutor(max_workers=len(clients)) as pool:
            report['measurements'].extend(pool.map(lambda c: measure(c, 'parallel', name, body), clients))
        save(); print(a.target, a.runtime, name, 'collected', flush=True)
    baseline = [s['cpu_seconds'] for m in report['measurements'] if m['workload'] == 'long_cpu' and m['phase'] == 'sequential' for s in m['result']['samples']]
    reference = statistics.median(baseline)
    report['boundary_reference'] = reference
    for factor in [.95, 1.05]:
        body = {'mode': 1, 'iterations': 300_000_000, 'samples': 10, 'cpu_budget': reference * factor}
        with ThreadPoolExecutor(max_workers=len(clients)) as pool:
            report['measurements'].extend(pool.map(lambda c: measure(c, f'boundary-{factor}', 'long_cpu', body), clients))
        save(); print(a.target, a.runtime, 'long boundary', factor, 'collected', flush=True)


if __name__ == '__main__':
    main()
