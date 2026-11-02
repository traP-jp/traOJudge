#!/usr/bin/env python3
"""Record the live local adapter at real time. No replay or synthetic states."""
import argparse
import json
from pathlib import Path
import time
import urllib.request
from playwright.sync_api import sync_playwright


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--port", type=int, default=8765)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    base = f"http://127.0.0.1:{args.port}"
    with sync_playwright() as p:
        browser = p.chromium.launch(executable_path="/usr/bin/chromium", args=["--no-sandbox"])
        context = browser.new_context(viewport={"width": 1440, "height": 1050},
            record_video_dir="work/tree-demo-video-raw", record_video_size={"width": 1440, "height": 1050}, locale="ja-JP")
        page = context.new_page()
        errors = []
        page.on("pageerror", lambda e: errors.append(str(e)))
        page.goto(base, wait_until="networkidle")
        page.wait_for_timeout(800)
        started = time.time()
        page.click("#submit")
        page.wait_for_function("window.demoError || window.demoSnapshots.some(s => s.cards[0].pipeline === 'cancelled' && !s.cards[0].existing_finished)", timeout=120000)
        assert not page.evaluate("window.demoError || null")
        page.screenshot(path=str(args.output / "tree-fail-fast-stopping.png"))
        page.wait_for_function("window.demoDone || window.demoError", timeout=180000)
        assert not page.evaluate("window.demoError || null"), page.evaluate("window.demoError")
        assert not errors, errors
        snapshots = page.evaluate("window.demoSnapshots")
        # Must visibly distinguish already issued work from all future dispatch.
        assert any(s["cards"][0]["pipeline"] == "cancelled" and
            not s["cards"][0]["existing_finished"] and s["cards"][0]["cases"][5]["status"] == "stopped"
            for s in snapshots), "did not observe cancellation before the already-issued run finished"
        page.screenshot(path=str(args.output / "tree-fail-fast-poster.png"))
        page.wait_for_timeout(3500)
        video = page.video
        context.close()
        raw = video.path()
        browser.close()
    with urllib.request.urlopen(base + "/evidence") as response:
        evidence = json.load(response)
    evidence.update(browser_snapshots=snapshots, recording={"speed": "1x", "cuts": False,
        "source": "live Chromium capture of actual AWS responses and read-only external-result observations",
        "capture_started_at_unix": started, "width": 1440, "height": 1050})
    Path("work/tree-demo-browser.json").write_text(json.dumps(evidence, indent=2) + "\n")
    Path("work/tree-demo-raw-path.txt").write_text(str(raw))
    print("Live browser recording and observed cancellation verified", flush=True)


if __name__ == "__main__":
    main()
