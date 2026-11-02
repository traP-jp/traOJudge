"""Summarize raw measurements without excluding slow or failing samples."""
import json
import math
import os
from pathlib import Path
import statistics
import random

os.environ.setdefault('MPLCONFIGDIR', str(Path(__file__).resolve().parents[3] / 'work/matplotlib'))
os.environ.setdefault('XDG_CACHE_HOME', str(Path(__file__).resolve().parents[3] / 'work/cache'))
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt


def stats(values):
    values = sorted(values)
    if not values:
        return None
    return {'n': len(values), 'median': statistics.median(values),
            'p95': values[math.ceil(.95 * len(values)) - 1], 'min': min(values), 'max': max(values),
            'cv': statistics.pstdev(values) / statistics.mean(values)}


def main():
    root = Path(__file__).resolve().parents[3]
    reports = [json.loads((root / f'work/fairness-{target}.json').read_text()) for target in ['microvm', 'ec2']]
    native = root / 'work/fairness-ec2-native.json'
    if native.exists():
        control = json.loads(native.read_text()); control['target'] = 'ec2-native'
        reports.append(control)
    summary = {}
    for report in reports:
        target = report['target']
        rows = report['measurements']
        values = {}
        for workload in ['cpu', 'memory', 'children']:
            phases = {}
            for phase in ['sequential', 'parallel']:
                samples = [s for m in rows if m['phase'] == phase and m['workload'] == workload for s in m['result']['samples']]
                phases[phase] = {'cpu': stats([s['cpu_seconds'] for s in samples]),
                                 'wall': stats([s['wall_seconds'] for s in samples]),
                                 'verdicts': {v: sum(s['verdict'] == v for s in samples) for v in sorted(set(s['verdict'] for s in samples))}}
            ratio = phases['parallel']['cpu']['p95'] / phases['sequential']['cpu']['p95'] - 1
            values[workload] = {**phases, 'p95_cpu_change': ratio, 'within_5_percent': abs(ratio) <= .05}
        boundary = {}
        for phase in sorted(set(m['phase'] for m in rows if m['phase'].startswith('boundary'))):
            samples = [s for m in rows if m['phase'] == phase for s in m['result']['samples']]
            boundary[phase] = {'n': len(samples), 'tle': sum(s['verdict'] == 'TLE' for s in samples),
                               'ok': sum(s['verdict'] == 'OK' for s in samples),
                               'other': sum(s['verdict'] not in ['OK', 'TLE'] for s in samples)}
        environments = [m['result']['environment'] for m in rows]
        outputs = [s['output'] for m in rows if m['workload'] == 'cpu' and m['phase'] in ['sequential', 'parallel'] for s in m['result']['samples']]
        summary[target] = {'workloads': values, 'boundary': boundary,
            'client_latency': stats([m['result']['client_seconds'] for m in rows if m['phase'] == 'latency']),
            'child_to_single_cpu_ratio': values['children']['sequential']['cpu']['median'] / values['cpu']['sequential']['cpu']['median'],
            'architectures': sorted(set(e['architecture'] for e in environments)),
            'compiler_versions': sorted(set(e['compiler'] for e in environments)),
            'binary_hashes': sorted(set(e['binary_sha256'] for e in environments)),
            'cpu_models': sorted(set(json.dumps(e['cpu_model']) for e in environments)),
            'constant_cpu_fixture_output': len(set(outputs)) == 1 and outputs[0].strip() == '8726995723297866208',
            'watchdog': [s for m in rows if m['phase'] == 'watchdog' for s in m['result']['samples']],
            'abort': [s for m in rows if m['phase'] == 'abort' for s in m['result']['samples']]}
        cpu_values = [round(s['cpu_seconds'] * 1_000_000) for m in rows
            if m['phase'] in ['sequential', 'parallel'] and m['workload'] == 'cpu'
            for s in m['result']['samples']]
        summary[target]['observed_cpu_quantization_us'] = math.gcd(*cpu_values)
        if report.get('cold_launches'):
            summary[target]['run_to_first_result_seconds'] = [r['run_to_first_result_seconds'] for r in report['cold_launches']]
    summary['long_probes'] = {}
    for file in sorted((root / 'work').glob('fairness-long-*.json')):
        r = json.loads(file.read_text()); row = {}
        for name in ['long_cpu', 'long_memory']:
            row[name] = {}
            for phase in ['sequential', 'parallel']:
                samples = [s['cpu_seconds'] for m in r['measurements']
                    if m['phase'] == phase and m['workload'] == name for s in m['result']['samples']]
                row[name][phase] = stats(samples)
        row['boundary'] = {}
        for phase in ['boundary-0.95', 'boundary-1.05']:
            samples = [s for m in r['measurements'] if m['phase'] == phase for s in m['result']['samples']]
            row['boundary'][phase] = {'n': len(samples), 'tle': sum(s['verdict'] == 'TLE' for s in samples),
                'ok': sum(s['verdict'] == 'OK' for s in samples)}
        summary['long_probes'][file.stem] = row
    queued = root / 'work/fairness-queue.json'
    if queued.exists():
        r = json.loads(queued.read_text())
        summary['queued_compute_zero'] = {k: r[k] for k in ['definition', 'latency', 'cpu', 'final_states', 'faults'] if k in r}
    destination = root / 'docs/services/fairness-summary.json'
    destination.write_text(json.dumps(summary, indent=2, ensure_ascii=False) + '\n')

    fig, axes = plt.subplots(2, 3, figsize=(13, 8.5))
    for row, (field, label) in enumerate([('cpu_seconds', 'Reported CPU time (ms)'), ('wall_seconds', 'Execution wall time (ms)')]):
        for ax, workload in zip(axes[row], ['cpu', 'memory', 'children']):
            data, labels = [], []
            for report in reports:
                for phase in ['sequential', 'parallel']:
                    values = [s[field] * 1000 for m in report['measurements']
                        if m['phase'] == phase and m['workload'] == workload for s in m['result']['samples']]
                    data.append(values); labels.append(report['target'] + '\n' + phase + f'\nn={len(values)}')
            ax.boxplot(data, tick_labels=labels, showfliers=False)
            rng = random.Random(31)
            for i, values in enumerate(data, 1):
                ax.scatter([i + rng.uniform(-.13, .13) for _ in values], values, s=5, alpha=.35)
            ax.set_title(workload); ax.set_ylabel(label); ax.grid(axis='y', alpha=.25)
            ax.tick_params(axis='x', labelsize=7)
    fig.suptitle('Same ARM64 C fixture: reported CPU vs wall time (all samples; native EC2 is a diagnostic control)')
    fig.tight_layout()
    fig.savefig(root / 'docs/services/diagrams/fairness-distribution.png', dpi=180)
    print(json.dumps(summary, ensure_ascii=False, indent=2))


if __name__ == '__main__':
    main()
