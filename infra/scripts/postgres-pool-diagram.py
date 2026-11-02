"""Render with caru-ini/stack-diagram's template, official icons and renderer."""
from pathlib import Path
import re,subprocess
root=Path(__file__).resolve().parents[2]
skill=root/'work/stack-diagram/skills/stack-diagram'
html=(skill/'assets/template.html').read_text().split('</style>')[0]
html=re.sub(r'@font-face\s*\{.*?\}','',html,flags=re.S)
html+='''
.canvas,svg.edges{width:1280px;height:840px}.node .role{font-size:12px}.node .name{font-size:14px}
</style></head><body><main class="page"><header class="head"><div><p class="kicker">traOJudge / disposable distributed workload proof</p><h1>PostgreSQL durable jobs + EC2 pool (0–5)</h1></div></header><div class="scroller"><div class="canvas">
<p style="position:absolute;left:30px;top:20px;font-size:15px">Distributed compile → run → check · commit receipt order · publish a contiguous terminal prefix</p>
<div class="group aws" style="left:210px;top:75px;width:1040px;height:690px"><div class="g-label"><img src="%%ICON_aws%%" alt="">AWS Cloud · ap-northeast-1</div></div>
<div class="group aws vpc" style="left:230px;top:125px;width:1000px;height:590px"><div class="g-label">VPC · no NAT gateway</div></div>
<div class="group aws private" style="left:250px;top:180px;width:460px;height:360px"><div class="g-label">Private subnets</div></div>
<div class="group aws public" style="left:745px;top:180px;width:475px;height:450px"><div class="g-label">Worker subnet · no inbound rules</div></div>
<div class="group ga-nested" style="left:770px;top:410px;width:270px;height:195px"><div class="g-label">Fresh gVisor</div></div>
<svg class="edges" viewBox="0 0 1280 840"><defs><marker id="open" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="6.5" markerHeight="6.5" orient="auto"><path d="M1 1 L9 5 L1 9" fill="none" stroke="context-stroke" stroke-width="1.5"/></marker></defs>
<path class="aws" d="M131 270 H319" marker-end="url(#open)"/><text class="el" x="220" y="259" text-anchor="middle">Invoke (IAM)</text>
<path class="aws" d="M391 270 H564" marker-end="url(#open)"/><text class="el" x="477" y="259" text-anchor="middle">commit / replay</text>
<path class="aws" d="M636 270 H814" marker-end="url(#open)"/><text class="el" x="725" y="259" text-anchor="middle">lease + fence</text>
<path class="aws" d="M886 270 H1054" marker-end="url(#open)"/><text class="el" x="970" y="259" text-anchor="middle">one node / host</text>
<path class="aws" d="M1090 346 V385 H1120 V444" marker-end="url(#open)"/><text class="el" x="1130" y="395">load root</text>
<path class="aws" d="M1084 480 H941" marker-end="url(#open)"/><text class="el" x="1012" y="469" text-anchor="middle">read-only</text>
<path class="aws" d="M1054 270 H1015 V395 H905 V444" marker-end="url(#open)"/><text class="el" x="915" y="384">execute</text>
<path class="aws" d="M869 480 H600 V366" marker-end="url(#open)"/><text class="el" x="610" y="420">commit result</text>
<path class="aws dash" d="M355 366 H235 V650 H735 V150 H850 V234" marker-end="url(#open)"/><text class="el" x="510" y="641" text-anchor="middle">wake / scale 0–5</text>
</svg>
<p style="position:absolute;left:270px;top:465px;width:250px;font-size:12px">Auto Scaling API via PrivateLink.<br>Recovery tick runs outside the EC2 pool.</p>
'''
for x,y,icon,name,role,bare,width in [
(95,240,'user','Submission','idempotent receipt',True,180),
(355,240,'lambda','AWS Lambda','Rust Engine / wake / recovery',True,200),
(600,240,'rds','Amazon RDS','order / DAG / lease / DLQ',True,200),
(850,240,'ec2','Amazon EC2','0–5 · SMT disabled',True,180),
(1090,240,'rust','Rust runner','trusted control process',False,200),
(905,450,'sandbox','Isolated execution','network / PID / RAM limits',False,250),
(1120,450,'nix','Pinned Nix','selected closure only',False,180)]:
 imgs=f'<img src="%%ICON_{icon}%%" alt="">'
 if icon=='user':imgs='<img class="lt" src="%%ICON_user%%" alt=""><img class="dk" src="%%ICON_user_dark%%" alt="">'
 tile='bare res' if icon=='user' else 'bare' if bare else ''
 html+=f'<div class="node" style="left:{x-width/2}px;top:{y}px;width:{width}px"><div class="tile {tile}">{imgs}</div><div class="name"><span>{name}</span></div><div class="role"><span>{role}</span></div></div>'
html+='''<p style="position:absolute;left:250px;top:727px;font-size:12px">DB + API endpoint remain on. Scale-to-zero applies to EC2. Cold startup and CPU fairness require measured acceptance.</p>
<p style="position:absolute;left:30px;top:800px;font-size:13px">Provisional design implemented for verification. Each guest gets a fresh sandbox; Rust owns Nix roots / cleanup; a host watchdog reaps orphans.</p>
</div></div><p class="foot">AWS Architecture Icons · svgl · devicon · Lucide · <a href="https://github.com/caru-ini/stack-diagram">made with stack-diagram</a></p></main></body></html>'''
draft=root/'work/postgres-pool-diagram.html';draft.write_text(html)
subprocess.run([str(root/'work/venv/bin/python'),str(skill/'scripts/build.py'),str(draft),str(root/'work/icons'),str(root/'docs/services/diagrams/postgres-pool.html')],check=True)
