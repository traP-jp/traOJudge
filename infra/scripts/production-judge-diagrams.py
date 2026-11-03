"""Production alternatives, using caru-ini/stack-diagram's template/build."""
from pathlib import Path
import re, subprocess

ROOT=Path(__file__).resolve().parents[2]
SKILL=ROOT/'work/stack-diagram/skills/stack-diagram'
OUT=ROOT/'docs/services/diagrams'

def node(x,y,icon,name,role,width=210):
    pictures=f'<img src="%%ICON_{icon}%%" alt="">'
    return f'<div class="node" style="left:{x-width/2}px;top:{y}px;width:{width}px"><div class="tile">{pictures}</div><div class="name"><span>{name}</span></div><div class="role"><span>{role}</span></div></div>'

def build(slug,title,neon,microvm):
    text=(SKILL/'assets/template.html').read_text().split('</style>')[0]
    text=re.sub(r'@font-face\s*\{.*?\}','',text,flags=re.S)
    text+='''
.canvas,svg.edges{width:1440px;height:860px}.node .name{font-size:15px}.node .role{font-size:12px}
</style></head><body><main class="page"><div class="scroller"><div class="canvas">
'''
    text+=f'<p style="position:absolute;left:30px;top:15px;font-size:22px">{title}</p>'
    text+='<p style="position:absolute;left:30px;top:52px;font-size:13px">Production selection · one submission per execution slot · durable intake before dispatch</p>'
    left=440 if neon else 390
    text+=f'<div class="group aws" style="left:{left}px;top:115px;width:{1380-left}px;height:620px"><div class="g-label"><img src="%%ICON_aws%%" alt="">AWS Cloud · no NAT / ALB / always-on controller</div></div>'
    if neon:
        text+='<div class="group ga-nested" style="left:90px;top:390px;width:320px;height:235px"><div class="g-label">Neon Cloud · Singapore / US / EU</div></div>'
    else:
        text+='<div class="group aws private" style="left:425px;top:390px;width:270px;height:235px"><div class="g-label">Private DB subnets · 2 AZs</div></div>'
    text+='<div class="group ga-nested" style="left:1080px;top:390px;width:270px;height:235px"><div class="g-label">Fresh sandbox</div></div>'
    text+='''<svg class="edges" viewBox="0 0 1440 860"><defs><marker id="open" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="6.5" markerHeight="6.5" orient="auto"><path d="M1 1 L9 5 L1 9" fill="none" stroke="context-stroke" stroke-width="1.5"/></marker></defs>
<path class="aws" d="M136 240 H524" marker-end="url(#open)"/><text class="el" x="330" y="228" text-anchor="middle">submit / idempotency key</text>
<path class="aws" d="M596 240 H814" marker-end="url(#open)"/><text class="el" x="705" y="228" text-anchor="middle">durable intent</text>
<path class="aws" d="M886 240 H1174" marker-end="url(#open)"/><text class="el" x="1030" y="228" text-anchor="middle">bind / start once</text>
<path class="aws" d="M1174 266 H1020 V365 H850 V342" marker-end="url(#open)"/><text class="el" x="1030" y="350">result / stop</text>
<path class="aws" d="M1210 342 V444" marker-end="url(#open)"/><text class="el" x="1220" y="376">execute</text>
<path class="aws" d="M850 342 V444" marker-end="url(#open)"/><text class="el" x="860" y="428">write once</text>
'''
    returnx=445 if neon else 705
    text+=f'<path class="aws" d="M814 480 H{returnx} V675 H100 V342" marker-end="url(#open)"/><text class="el" x="290" y="665" text-anchor="middle">progress / final snapshot</text>'
    dbx=250 if neon else 560
    text+=f'<path class="aws" d="M814 266 H785 V365 H{dbx} V444" marker-end="url(#open)"/><text class="el" x="640" y="354" text-anchor="middle">receipt + queue TX</text>'
    text+='</svg>'
    text+=node(100,210,'user','User','submit / reconnect',170)
    text+=node(560,210,'lambda','AWS Lambda','Rust API · pay per request')
    text+=node(850,210,'sfn','AWS Step Functions','Standard · wake / supervise',250)
    text+=node(1210,210,'lambda' if microvm else 'ec2','AWS Lambda MicroVMs' if microvm else 'Amazon EC2','0–5 slots · ARM64' if microvm else '0–5 · C7i / pinned AMI',255)
    text+=node(dbx,450,'neon' if neon else 'rds','Neon Scale' if neon else 'Amazon Aurora','PGMQ · queue / ordering / DLQ',290 if neon else 220)
    text+=node(850,450,'s3','Amazon S3','immutable intake / start / terminal',280)
    text+=node(1210,450,'sandbox','Isolated execution','Nix closure · no network / keys',250)
    notes=(
        'Database compute can suspend. Idle storage remains billed. Empty queues are not polled through PostgreSQL.',
        'A lease is not permission to rerun. Ambiguous execution goes to fencing / DLQ; final results are immutable.',
        'MicroVM CPU model / burst is not fixed by the public API. Cold 10s is a target, not a service guarantee.' if microvm else
        'CPU placement is controlled on the host. Cold EC2 startup is outside the 10s warm target.',
    )
    for y,line in zip([745,775,805],notes):
        text+=f'<p style="position:absolute;left:30px;top:{y}px;font-size:13px">{line}</p>'
    text+='</div></div><p class="foot">AWS Architecture Icons · svgl · Lucide · <a href="https://github.com/caru-ini/stack-diagram">made with stack-diagram</a></p></main></body></html>'
    draft=ROOT/f'work/{slug}.html';draft.write_text(text)
    subprocess.run([str(ROOT/'work/venv/bin/python'),str(SKILL/'scripts/build.py'),str(draft),str(ROOT/'work/icons'),str(OUT/f'{slug}.html')],check=True)

if __name__=='__main__':
    build('production-neon-ec2','A · Neon Scale + PGMQ + EC2',True,False)
    build('production-aurora-ec2','B · Aurora Serverless v2 + PGMQ + EC2',False,False)
    build('production-neon-microvm','C · Neon Scale + PGMQ + Lambda MicroVMs',True,True)
