#!/usr/bin/env python3
"""Record an uncut live browser demo and assert every displayed progress state."""
import argparse
import asyncio
import json
from pathlib import Path
import subprocess
import urllib.request

from playwright.async_api import async_playwright


async def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--url", default="http://127.0.0.1:8765")
    parser.add_argument("--output", type=Path, default=Path("docs/services/demos"))
    parser.add_argument("--raw", type=Path, default=Path("work/demo-video-raw"))
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    args.raw.mkdir(parents=True, exist_ok=True)
    console_errors = []
    async with async_playwright() as p:
        browser = await p.chromium.launch(executable_path="/usr/bin/chromium", args=["--no-sandbox"])
        context = await browser.new_context(viewport={"width": 1440, "height": 1040},
            record_video_dir=str(args.raw), record_video_size={"width": 1440, "height": 1040},
            timezone_id="Asia/Tokyo", locale="ja-JP")
        page = await context.new_page()
        page.on("pageerror", lambda error: console_errors.append(str(error)))
        await page.goto(args.url)
        await page.evaluate("document.fonts.ready")
        await page.wait_for_timeout(2200)
        for trial in range(2):
            await page.locator("#submit").click()
            for progress in ["WJ", "1/3", "2/3", "3/3", "AC"]:
                await page.wait_for_function("p => window.demo.observed.includes(p)", arg=progress, timeout=90000)
                if trial == 0 and progress in ["WJ", "1/3", "AC"]:
                    await page.screenshot(path=str(args.raw / (progress.replace("/", "-") + ".png")))
            await page.wait_for_function("n => window.demo.completed.length === n", arg=trial + 1, timeout=30000)
            print(f"Submission {trial + 1}: WJ -> 1/3 -> 2/3 -> 3/3 -> AC observed", flush=True)
            await page.wait_for_timeout(2400)
        completed = await page.evaluate("window.demo.completed")
        assert not console_errors, "browser JavaScript error"
        await page.screenshot(path=str(args.output / "judge-flow-poster.png"))
        video_path = await page.video.path()
        await context.close()
        await browser.close()
    with urllib.request.urlopen(args.url + "/evidence") as response:
        evidence = json.load(response)
    assert [r["submission"] for r in evidence["receipts"]] == [1, 2]
    checks = {"two_ordered_submissions": True, "browser_javascript_errors_absent": True}
    for i, row in enumerate(completed, 1):
        assert row["observed"] == ["WJ", "1/3", "2/3", "3/3", "AC"], row["observed"]
        audit = evidence["audits"][row["request_id"]]
        assert all(audit["checks"].values())
        trace = evidence["traces"][row["request_id"]]
        assert [v["view"]["completed_cases"] for v in trace] == sorted(v["view"]["completed_cases"] for v in trace)
        checks[f"submission_{i}_all_display_states_observed"] = True
        checks[f"submission_{i}_8_isolated_nodes_and_artifact_summary_verified"] = True
    report = {"scenario": "live-browser-sequential-compile-run-check-summarize", "region": "ap-northeast-1",
        "recording": {"speed": "1x", "cuts": False, "source": "Playwright live Chromium capture; actual AWS API responses",
                      "native_case_delay_ms": 100}, "checks": checks, "browser_completed": completed, **evidence}
    (args.output / "judge-flow-evidence.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n")
    subprocess.run(["ffmpeg", "-y", "-loglevel", "error", "-i", str(video_path), "-c:v", "libx264",
        "-preset", "medium", "-crf", "22", "-pix_fmt", "yuv420p", "-movflags", "+faststart", "-an",
        str(args.output / "judge-flow.mp4")], check=True)
    print("PASS: live video, poster, and sanitized evidence saved", flush=True)


if __name__ == "__main__":
    asyncio.run(main())
