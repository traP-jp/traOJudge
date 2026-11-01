#!/usr/bin/env python3
"""Generate the source HTML for caru-ini/stack-diagram's build.py."""
import argparse
from pathlib import Path
import re

parser = argparse.ArgumentParser()
parser.add_argument("skill", type=Path)
parser.add_argument("output", type=Path)
args = parser.parse_args()
template = (args.skill / "assets/template.html").read_text()
style = re.search(r"<style>(.*?)</style>", template, re.S).group(1)
style = style.replace("width: 1000px; height: 540px", "width: 1500px; height: 960px")
style = style.replace("width:1000px; height:540px", "width:1500px; height:960px")
groups = """
<div class="group aws" style="left:200px;top:24px;width:1260px;height:912px">
 <div class="g-label"><img class="lt" src="%%ICON_aws%%" alt=""><img class="dk" src="%%ICON_aws_dark%%" alt="">AWS Cloud · ap-northeast-1</div></div>
<div class="group aws vpc" style="left:610px;top:330px;width:240px;height:580px">
 <div class="g-label"><img src="%%ICON_vpc%%" alt="">Execution VPC</div></div>
<div class="group aws public" style="left:634px;top:380px;width:192px;height:506px">
 <div class="g-label">Public subnets</div></div>
"""
nodes = [
    (100, 120, 'user', 'Operator / CLI', 'IAM-authenticated proof'),
    (290, 120, 'lambda', 'AWS Lambda', 'acceptance / read API'),
    (510, 120, 'dynamodb', 'Amazon DynamoDB', 'ordering + receipt outbox'),
    (730, 120, 'lambda', 'AWS Lambda', 'receipt relay / wake'),
    (950, 120, 'sqs', 'Amazon SQS FIFO', 'per-run control'),
    (1170, 120, 'lambda', 'AWS Lambda', 'Rust workflow engine'),
    (1170, 390, 's3', 'Amazon S3', 'checkpoint + dispatch outbox'),
    (950, 390, 'sqs', 'Amazon SQS', 'ready node requests'),
    (730, 440, 'ec2', 'Amazon EC2', 'trusted agent / 2 slots'),
    (510, 390, 'autoscaling', 'EC2 Auto Scaling', 'warm 1 / idle 0'),
    (290, 390, 'lambda', 'AWS Lambda', 'wake / idle reconcile'),
    (290, 720, 'ecr', 'Amazon ECR', 'worker image cache'),
    (730, 720, 'sandbox', 'Offline sandbox', 'fresh gVisor / each node'),
    (1050, 720, 's3', 'Amazon S3', 'versioned artifacts / results'),
]
markup = []
for x, y, icon, name, role in nodes:
    image = f'<img src="%%ICON_{icon}%%" alt="">'
    if icon == 'user':
        image = f'<img class="lt" src="%%ICON_user%%" alt=""><img class="dk" src="%%ICON_user_dark%%" alt="">'
    markup.append(f'<div class="node" style="left:{x-90}px;top:{y}px;width:180px"><div class="tile {"" if icon=="sandbox" else "bare"} {"res" if icon=="user" else ""}">{image}</div><div class="name"><span>{name}</span></div><div class="role"><span>{role}</span></div></div>')
edges = []
def edge(d, label='', x=0, y=0, dash=False):
    edges.append(f'<path class="aws {"dash" if dash else ""}" d="{d}" marker-end="url(#open)"/>')
    if label:
        edges.append(f'<text class="el" x="{x}" y="{y}" text-anchor="middle">{label}</text>')
edge('M134 150 H256', 'HTTPS / SigV4', 195, 140)
edge('M324 150 H476', 'atomic accept', 400, 140)
edge('M544 150 H696', 'Streams', 620, 140)
edge('M764 150 H916', 'outbox', 840, 140)
edge('M984 150 H1136', 'serialized events', 1060, 140)
edge('M1170 224 V384', 'CAS snapshot', 1242, 278)
edge('M1136 190 H1090 V320 H950 V384', 'after checkpoint', 985, 310)
edge('M916 420 H870 V470 H764', 'long poll', 850, 410)
edge('M730 224 V280 H290 V384', 'wake', 490, 270)
edge('M324 420 H476', 'desired 0 / 1', 400, 410)
edge('M544 420 H730 V434', 'capacity', 602, 410)
edge('M730 544 V714', 'fresh sandbox', 796, 637)
edge('M290 824 V858 H590 V470 H696', 'boot pre-pull', 430, 848, True)
edge('M764 490 H870 V680 H1050 V714', 'host-only S3', 1000, 670)
edge('M764 460 H860 V300 H1380 V80 H950 V114', 'durable result', 1260, 68)
edge('M1204 420 H1320 V240 H290 V224', 'read saved progress / final result', 485, 232)
svg = '<svg class="edges" viewBox="0 0 1500 960"><defs><marker id="open" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="6.5" markerHeight="6.5" orient="auto-start-reverse"><path d="M1 1 L9 5 L1 9" fill="none" stroke="context-stroke" stroke-width="1.5"/></marker></defs>' + ''.join(edges) + '</svg>'
notes = """<div style="position:absolute;left:890px;top:838px;width:470px;font-family:var(--font-mono);font-size:11px;line-height:1.8;color:var(--color-muted)">Guest: no network · no AWS credentials · read-only root<br>Host handles S3 / SQS; job state survives host loss.<br>Warm P95 8.511s · 2 submissions / 3 cases · idle 0 VMs</div>"""
html = f'''<!DOCTYPE html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><title>traOJudge · durable native execution</title><style>{style}</style></head><body><div class="page">
<header class="head"><div><div class="kicker">traOJudge / infrastructure proof</div><h1>Ordered acceptance. Durable workflow. Isolated native execution.</h1></div><p class="lede">Warm execution under 10 seconds in the measured fixture. Scale to zero between sessions; cold boot measured separately.</p></header>
<div class="scroller"><div class="canvas">{groups}{svg}{''.join(markup)}{notes}</div></div>
<div class="foot"><span>Control and execution lifetimes are independent of the reader.</span><span>icons: AWS Architecture Icons / Lucide · fonts: Geist / JetBrains Mono (SIL OFL 1.1) · made with <a href="https://github.com/caru-ini/stack-diagram">stack-diagram</a></span></div></div></body></html>'''
args.output.write_text(html)
