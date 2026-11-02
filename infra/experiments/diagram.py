#!/usr/bin/env python3
"""Generate documentation with caru-ini/stack-diagram; no AWS discovery or calls.

Pass --skill-root pointing to that repository's skills/stack-diagram directory
and --icons to an official-icon cache created by its fetch_icons.py script.
"""
import argparse
import html
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--skill-root", type=Path, required=True)
    parser.add_argument("--icons", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    template = (args.skill_root / "assets/template.html").read_text()
    style = template.split("<style>", 1)[1].split("</style>", 1)[0]
    style += "\n.canvas,svg.edges{width:1480px;height:980px}.note{position:absolute;font-size:12px;line-height:1.8;color:var(--color-ink-2)}"
    parts = [f'<!DOCTYPE html><html lang="en"><head><meta charset="utf-8"><title>traOJudge VM pool</title><style>{style}</style></head><body><main class="page">',
        '<header class="head"><div><p class="kicker">traOJudge · AWS / current experiment · 2026-11-02</p><h1>Durable execution on a protected VM pool</h1></div><p class="lede">One node at a time per VM. Maximum five VMs. Policy is separate from the application adapters.</p></header>',
        '<div class="scroller"><div class="canvas">',
        '<div class="group aws" style="left:220px;top:40px;width:1220px;height:900px"><div class="g-label"><img src="%%ICON_aws%%" alt="">AWS Cloud · ap-northeast-1</div></div>',
        '<div class="group aws vpc" style="left:250px;top:650px;width:1130px;height:235px"><div class="g-label"><img src="%%ICON_vpc%%" alt="">VPC · public subnet · single AZ · scale-in protected on launch</div></div>',
        '<svg class="edges" width="1480" height="980" viewBox="0 0 1480 980"><defs><marker id="open" markerWidth="8" markerHeight="8" refX="7" refY="4" orient="auto"><path d="M1 1 L7 4 L1 7" fill="none" stroke="context-stroke"/></marker></defs>']

    def edge(path, label="", x=0, y=0, dashed=False):
        parts.append(f'<path class="aws{" dash" if dashed else ""}" d="{path}" marker-end="url(#open)"/>')
        if label:
            parts.append(f'<text class="el" x="{x}" y="{y}" text-anchor="middle">{html.escape(label)}</text>')

    edge("M124 200 H316", "submit / JSON poll", 220, 190)
    edge("M384 200 H536", "transaction", 460, 190)
    edge("M604 200 H756", "receipt stream", 680, 190)
    edge("M824 200 H976", "outbox", 900, 190)
    edge("M1044 200 H1196", "FIFO per run", 1120, 190)
    edge("M1230 276 V320 H350 V394", "DB + view + outbox checkpoint", 760, 310)
    edge("M1230 276 V340 H570 V394", "durable state → modeled demand", 870, 332)
    edge("M1230 276 V394", "dispatch", 1280, 365)
    edge("M604 430 H756", "safe target metric", 680, 420)
    edge("M824 430 H976", "alarms / policies", 900, 420)
    edge("M90 506 V560 H570 V506", "compiled pool.json · immutable deployment", 330, 551, True)
    edge("M1010 506 V644", "exact 0…5", 1060, 570, True)
    edge("M350 506 V704", "version + hash", 410, 576)
    edge("M1230 506 V620 H350", "reserved receive → execution → cleanup → ACK", 870, 610)
    for center in [350, 570, 790, 1010, 1230]:
        edge(f"M{center} 620 V704")
    parts.append("</svg>")

    def node(center, top, icon, title, role):
        parts.append(f'<div class="node" style="left:{center-86}px;top:{top}px;width:172px"><div class="tile bare"><img src="%%ICON_{icon}%%" alt=""></div><div class="name"><span>{html.escape(title)}</span></div><div class="role"><span>{html.escape(role)}</span></div></div>')

    node(90, 170, "user", "Client", "JSON snapshot polling")
    node(350, 170, "lambda", "AWS Lambda", "accept / read · IAM fixture")
    node(570, 170, "dynamodb", "Amazon DynamoDB", "receipt + ordering")
    node(790, 170, "lambda", "AWS Lambda", "receipt outbox relay")
    node(1010, 170, "sqs", "Amazon SQS", "control FIFO + DLQ")
    node(1230, 170, "lambda", "AWS Lambda", "Engine checkpoint writer")
    node(90, 400, "policy", "Pool policy", "pure decisions + schema")
    node(350, 400, "s3", "Amazon S3", "artifacts / durable state")
    node(570, 400, "lambda", "AWS Lambda", "capacity / drain adapter")
    node(790, 400, "cloudwatch", "Amazon CloudWatch", "60s Maximum · missing ≠ 0")
    node(1010, 400, "autoscaling", "Amazon EC2 Auto Scaling", "ExactCapacity · max 5")
    node(1230, 400, "sqs", "Amazon SQS", "node queue → NodeDLQ → IE")
    for i, center in enumerate([350, 570, 790, 1010, 1230], 1):
        node(center, 710, "ec2", f"Amazon EC2 · VM {i}", "1 offline gVisor node")
    parts.extend(['<p class="note" style="left:270px;top:840px">The five VM symbols show pool capacity, not five permanently running hosts. Each host consumes sequentially.</p>',
        '<p class="note" style="left:270px;top:897px">Retire irreversibly after poll / execution / cleanup. Forced loss → bounded retry. Boot-stuck health recovery remains a blocker.</p>',
        '</div></div><p class="foot">Icons: AWS Architecture Icons / Lucide. Fonts: Geist / JetBrains Mono. Logos belong to their owners. · <a href="https://github.com/caru-ini/stack-diagram">made with stack-diagram</a></p></main></body></html>'])
    args.out.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory() as temp:
        draft = Path(temp) / "draft.html"
        draft.write_text("\n".join(parts))
        subprocess.run([sys.executable, str(args.skill_root / "scripts/build.py"), str(draft), str(args.icons), str(args.out),
            "--fonts", str(args.skill_root / "assets/fonts")], check=True)
    for name in ["OFL-Geist.txt", "OFL-JetBrainsMono.txt"]:
        license_text = (args.skill_root / "assets/fonts" / name).read_text()
        (args.out.parent / name).write_text("\n".join(line.rstrip() for line in license_text.splitlines()) + "\n")
    shutil.copyfile(args.skill_root / "LICENSE", args.out.parent / "stack-diagram-LICENSE.txt")


if __name__ == "__main__":
    main()
