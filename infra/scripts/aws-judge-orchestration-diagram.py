"""Render the AWS-only selection using caru-ini/stack-diagram."""
from pathlib import Path
import re,subprocess

ROOT=Path(__file__).resolve().parents[2]
SKILL=ROOT/'work/stack-diagram/skills/stack-diagram'
text=(SKILL/'assets/template.html').read_text().split('</style>')[0]
text=re.sub(r'@font-face\s*\{.*?\}','',text,flags=re.S)
text+='''
.canvas,svg.edges{width:1380px;height:900px}.node .name{font-size:15px}.node .role{font-size:12px}
</style></head><body><main class="page"><div class="scroller"><div class="canvas">
<p style="position:absolute;left:30px;top:18px;font-size:24px">AWS judge · author-defined workflows as data</p>
<p style="position:absolute;left:30px;top:60px;font-size:14px">Ordered receipts · durable orchestration · isolated execution · live progress</p>
<div class="group aws" style="left:215px;top:120px;width:1130px;height:650px"><div class="g-label"><img src="%%ICON_aws%%" alt="">AWS Cloud</div></div>
<div class="group ga-nested" style="left:1060px;top:430px;width:270px;height:260px"><div class="g-label">0–5 hosts</div></div>
<svg class="edges" viewBox="0 0 1380 900"><defs><marker id="open" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="6.5" markerHeight="6.5" orient="auto"><path d="M1 1 L9 5 L1 9" fill="none" stroke="context-stroke" stroke-width="1.5"/></marker></defs>
<path class="aws" d="M136 230 H304" marker-end="url(#open)"/><text class="el" x="220" y="218" text-anchor="middle">submit / publish</text>
<path class="aws" d="M340 194 V160 H900 V194" marker-end="url(#open)"/><text class="el" x="620" y="150" text-anchor="middle">validate / publish author ASL</text>
<path class="aws" d="M376 230 H534" marker-end="url(#open)"/><text class="el" x="455" y="218" text-anchor="middle">atomic accept</text>
<path class="aws" d="M606 230 H864" marker-end="url(#open)"/><text class="el" x="735" y="218" text-anchor="middle">start relay</text>
<path class="aws" d="M936 230 H1164" marker-end="url(#open)"/><text class="el" x="1050" y="218" text-anchor="middle">submitJob.sync</text>
<path class="aws" d="M1200 342 V494" marker-end="url(#open)"/><text class="el" x="1210" y="410">schedule / retry</text>
<path class="aws" d="M864 266 H780 V355 H570 V342" marker-end="url(#open)"/><text class="el" x="710" y="345" text-anchor="middle">finalize once</text>
<path class="aws" d="M570 342 V420 H340 V494" marker-end="url(#open)"/><text class="el" x="450" y="408" text-anchor="middle">stream / publish</text>
<path class="aws" d="M304 530 H100 V342" marker-end="url(#open)"/><text class="el" x="110" y="440">live + snapshot</text>
<path class="aws" d="M570 342 V455 H900 V494" marker-end="url(#open)"/><text class="el" x="820" y="445" text-anchor="middle">failure outbox</text>
<path class="aws" d="M570 494 V470 H720 V390 H900 V342" marker-end="url(#open)"/><text class="el" x="800" y="403" text-anchor="middle">pinned inputs</text>
<path class="aws" d="M1200 650 V715 H570 V642" marker-end="url(#open)"/><text class="el" x="850" y="705" text-anchor="middle">attempt manifests</text>
<path class="aws" d="M1164 530 H1040 V375 H570 V342" marker-end="url(#open)"/><text class="el" x="1050" y="478">progress</text>
</svg>
'''
for x,y,icon,name,role,width in [
    (100,200,'user','User','submission / author flow',180),
    (340,200,'lambda','AWS Lambda','API / registration / short relays',220),
    (570,200,'ddb','Amazon DynamoDB','sequence / jobs / terminal / outbox',250),
    (900,200,'sfn','AWS Step Functions','Standard · supervisor + author ASL',270),
    (1200,200,'batch','AWS Batch','managed queue / scheduling / retry',250),
    (340,500,'appsync','AWS AppSync Events','WebSocket / auth / reconnect',220),
    (570,500,'s3','Amazon S3','flow data / input / artifacts',230),
    (900,500,'sqs','Amazon SQS','terminal failure queue',220),
    (1200,500,'ec2','Amazon EC2','trusted runner + fresh microVM',240),
]:
    text+=f'<div class="node" style="left:{x-width/2}px;top:{y}px;width:{width}px"><div class="tile"><img src="%%ICON_{icon}%%" alt=""></div><div class="name"><span>{name}</span></div><div class="role"><span>{role}</span></div></div>'
for y,line in [(790,'Author flow publication is an API/data operation. No per-problem repository or IaC deployment.'),
               (823,'Batch owns the work queue and worker retry. Final judgement is committed once; physical retries may occur.'),
               (856,'A small reconciler repairs pending starts, expired executions and failed DLQ delivery; it does not schedule nodes.')]:
    text+=f'<p style="position:absolute;left:30px;top:{y}px;font-size:13px">{line}</p>'
text+='</div></div><p class="foot">AWS Architecture Icons · <a href="https://github.com/caru-ini/stack-diagram">made with stack-diagram</a></p></main></body></html>'
draft=ROOT/'work/aws-judge-orchestration.html';draft.write_text(text)
subprocess.run([str(ROOT/'work/venv/bin/python'),str(SKILL/'scripts/build.py'),str(draft),str(ROOT/'work/icons'),str(ROOT/'docs/services/diagrams/aws-judge-orchestration.html')],check=True)
