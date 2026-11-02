"""Build the experiment topology with caru-ini/stack-diagram's template and scripts."""
from pathlib import Path
import re
import subprocess

root = Path(__file__).resolve().parents[2]
skill = root / 'work/stack-diagram/skills/stack-diagram'
template = (skill / 'assets/template.html').read_text().split('</style>')[0]
template = re.sub(r'@font-face\s*\{.*?\}', '', template, flags=re.S)
html = template + '''
.canvas,svg.edges{width:1040px;height:820px}.node .role{font-size:12px}.node .name{font-size:14px}
</style></head><body><main class="page"><header class="head"><div><p class="kicker">traOJudge / comparative experiment</p><h1>Submission-level execution: fairness and cold latency</h1></div></header><div class="scroller"><div class="canvas">
<p style="position:absolute;left:35px;top:30px;font-size:16px">Same ARM64 C fixture · same compiler flags · sequential and parallel probes</p>
<div class="group aws" style="left:230px;top:90px;width:800px;height:660px"><div class="g-label"><img src="%%ICON_aws%%" alt="">AWS Cloud · ap-northeast-1</div></div>
<div class="group aws vpc" style="left:540px;top:440px;width:220px;height:210px"><div class="g-label">VPC · public subnet</div></div>
<svg class="edges" viewBox="0 0 1040 820"><defs><marker id="open" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="6.5" markerHeight="6.5" orient="auto"><path d="M1 1 L9 5 L1 9" fill="none" stroke="context-stroke" stroke-width="1.5"/></marker></defs>
<path class="aws" d="M144 380 H200 V210 H326" marker-end="url(#open)"/><text class="el" x="210" y="300">CDK create</text>
<path class="aws" d="M394 210 H616" marker-end="url(#open)"/><text class="el" x="505" y="198" text-anchor="middle">launch + first request</text>
<path class="aws" d="M110 344 V140 H650 V174" marker-end="url(#open)"/><text class="el" x="470" y="133" text-anchor="middle">HTTPS · repeat / parallel</text>
<path class="aws" d="M360 286 V320 H900 V354" marker-end="url(#open)"/><text class="el" x="745" y="311" text-anchor="middle">launch evidence</text>
<path class="aws" d="M200 380 V530 H326" marker-end="url(#open)"/><text class="el" x="210" y="475">submit</text>
<path class="aws" d="M394 530 H616" marker-end="url(#open)"/><text class="el" x="505" y="518" text-anchor="middle">one submission</text>
<path class="aws" d="M684 530 H900 V466" marker-end="url(#open)"/><text class="el" x="810" y="518" text-anchor="middle">durable result</text>
<path class="aws" d="M900 466 V690 H110 V456" marker-end="url(#open)"/><text class="el" x="500" y="681" text-anchor="middle">read result · compare CPU time and end-to-end latency</text>
</svg>
'''
for x, y, icon, name, role in [
    (110, 350, 'user', 'Test client', 'measure / assert'),
    (360, 180, 'lambda', 'AWS Lambda', 'CloudFormation owner'),
    (650, 180, 'lambda', 'AWS Lambda MicroVMs', '3 fresh VMs · no AWS role'),
    (360, 500, 'sqs', 'Amazon SQS', 'submission queue + DLQ'),
    (650, 500, 'ec2', 'Amazon EC2', '2 × c7g.large / gVisor'),
    (900, 360, 's3', 'Amazon S3', 'measurement evidence'),
]:
    imgs = f'<img src="%%ICON_{icon}%%" alt="">'
    if icon == 'user':
        imgs = '<img class="lt" src="%%ICON_user%%" alt=""><img class="dk" src="%%ICON_user_dark%%" alt="">'
    tile = 'bare res' if icon == 'user' else 'bare'
    html += f'<div class="node" style="left:{x-100}px;top:{y}px;width:200px"><div class="tile {tile}">{imgs}</div><div class="name"><span>{name}</span></div><div class="role"><span>{role}</span></div></div>'
html += '''<p style="position:absolute;left:35px;top:775px;font-size:14px">Allowlisted fixtures, not a production judge. CloudFormation owns creation and teardown of both alternatives.</p>
</div></div><p class="foot">AWS Architecture Icons · <a href="https://github.com/caru-ini/stack-diagram">made with stack-diagram</a></p></main></body></html>'''
draft = root / 'work/fairness-diagram-draft.html'
draft.write_text(html)
subprocess.run([str(root / 'work/venv/bin/python'), str(skill / 'scripts/build.py'), str(draft),
    str(root / 'work/icons'), str(root / 'docs/services/diagrams/fairness-comparison.html')], check=True)
