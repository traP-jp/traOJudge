"""Render the AWS-only selection using caru-ini/stack-diagram."""
from pathlib import Path
import re
import subprocess

ROOT = Path(__file__).resolve().parents[2]
SKILL = ROOT / 'work/stack-diagram/skills/stack-diagram'
text = (SKILL / 'assets/template.html').read_text().split('</style>')[0]
text = re.sub(r'@font-face\s*\{.*?\}', '', text, flags=re.S)
text += '''
.canvas,svg.edges{width:1480px;height:1100px}.node .name{font-size:15px}.node .role{font-size:12px}
</style></head><body><main class="page"><div class="scroller"><div class="canvas">
<p style="position:absolute;left:30px;top:18px;font-size:24px">AWS judge · author-defined workflows as data</p>
<p style="position:absolute;left:30px;top:60px;font-size:14px">Ordered receipts · durable orchestration · isolated execution · live progress</p>
<div class="group aws" style="left:250px;top:130px;width:1200px;height:830px"><div class="g-label"><img src="%%ICON_aws%%" alt="">AWS Cloud</div></div>
<div class="group ga-nested" style="left:1160px;top:440px;width:270px;height:250px"><div class="g-label">0–5 hosts</div></div>
<svg class="edges" viewBox="0 0 1480 1100"><defs><marker id="open" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="6.5" markerHeight="6.5" orient="auto"><path d="M1 1 L9 5 L1 9" fill="none" stroke="context-stroke" stroke-width="1.5"/></marker></defs>
'''
# Every connection is one straight segment; the positions follow the topology.
edges = [
    ('M136 230 H364', 250, 218, 'submit / publish'),
    ('M436 230 H964', 700, 218, 'validate / publish author ASL'),
    ('M400 342 L700 494', 520, 390, 'atomic accept'),
    ('M720 494 L960 342', 920, 405, 'start relay'),
    ('M920 342 L680 494', 720, 420, 'finalize once'),
    ('M1036 230 H1264', 1150, 218, 'submitJob.sync'),
    ('M1300 342 V494', 1370, 410, 'schedule / retry'),
    ('M1264 530 H736', 1130, 518, 'progress'),
    ('M664 530 H436', 550, 518, 'stream / publish'),
    ('M400 494 L100 342', 220, 441, 'live + snapshot'),
    ('M700 642 V794', 778, 720, 'failure outbox'),
    ('M1000 794 V342', 1066, 690, 'pinned inputs'),
    ('M1300 642 L1000 794', 1190, 742, 'attempt manifests'),
]
for path, x, y, label in edges:
    text += f'<path class="aws" d="{path}" marker-end="url(#open)"/><text class="el" x="{x}" y="{y}" text-anchor="middle">{label}</text>'
text += '</svg>'
for x, y, icon, name, role, width in [
    (100, 200, 'user', 'User', 'submission / author flow', 180),
    (400, 200, 'lambda', 'AWS Lambda', 'API / registration / short relays', 220),
    (1000, 200, 'sfn', 'AWS Step Functions', 'Standard · supervisor + author ASL', 270),
    (1300, 200, 'batch', 'AWS Batch', 'managed queue / scheduling / retry', 250),
    (400, 500, 'appsync', 'AWS AppSync Events', 'WebSocket / auth / reconnect', 220),
    (700, 500, 'ddb', 'Amazon DynamoDB', 'sequence / jobs / terminal / outbox', 250),
    (1300, 500, 'ec2', 'Amazon EC2', 'trusted runner + fresh microVM', 240),
    (700, 800, 'sqs', 'Amazon SQS', 'terminal failure queue', 220),
    (1000, 800, 's3', 'Amazon S3', 'flow data / input / artifacts', 230),
]:
    tile_class = 'tile' if icon == 'user' else 'tile bare'
    text += f'<div class="node" style="left:{x-width/2}px;top:{y}px;width:{width}px"><div class="{tile_class}"><img src="%%ICON_{icon}%%" alt=""></div><div class="name"><span>{name}</span></div><div class="role"><span>{role}</span></div></div>'
for y, line in [
    (980, 'Author flow publication is an API/data operation. No per-problem repository or IaC deployment.'),
    (1013, 'Batch owns the work queue and worker retry. Final judgement is committed once; physical retries may occur.'),
    (1046, 'A small reconciler repairs pending starts, expired executions and failed DLQ delivery; it does not schedule nodes.'),
]:
    text += f'<p style="position:absolute;left:30px;top:{y}px;font-size:13px">{line}</p>'
text += '</div></div><p class="foot">AWS Architecture Icons · <a href="https://github.com/caru-ini/stack-diagram">made with stack-diagram</a></p></main></body></html>'
draft = ROOT / 'work/aws-judge-orchestration.html'
draft.write_text(text)
subprocess.run([
    str(ROOT / 'work/venv/bin/python'), str(SKILL / 'scripts/build.py'),
    str(draft), str(ROOT / 'work/icons'),
    str(ROOT / 'docs/services/diagrams/aws-judge-orchestration.html'),
], check=True)
