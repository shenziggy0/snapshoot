#!/usr/bin/env python3
"""Wait for the Snapshoot smoke run and assert the captured frames."""

import argparse
import json
import subprocess
import sys
import time
from pathlib import Path

from PIL import Image

try:
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
    sys.stderr.reconfigure(encoding="utf-8", errors="replace")
except Exception:
    pass


def log(directory: Path, message: str) -> None:
    line = message.rstrip()
    try:
        print(line, flush=True)
    except UnicodeEncodeError:
        print(line.encode("ascii", "backslashreplace").decode("ascii"), flush=True)
    with (directory / "smoke-assert.log").open("a", encoding="utf-8") as handle:
        handle.write(line + "\n")


def fail(directory: Path, message: str) -> None:
    log(directory, "FAIL " + message)
    error = directory / "smoke-error"
    if error.exists():
        log(directory, "app error: " + error.read_text(encoding="utf-8", errors="replace"))
    sys.exit(1)


def wait_for(directory: Path, name: str, timeout: float) -> bool:
    deadline = time.time() + timeout
    target = directory / name
    while time.time() < deadline:
        if (directory / "smoke-error").exists() and not target.exists():
            return False
        if target.exists():
            return True
        time.sleep(1)
    return target.exists()


def samples(path: Path):
    image = Image.open(path).convert("RGB")
    width, height = image.size
    step_x = max(1, width // 160)
    step_y = max(1, height // 100)
    pixels = [
        image.getpixel((x, y))
        for y in range(0, height, step_y)
        for x in range(0, width, step_x)
    ]
    return image.size, pixels


def luminance_stats(pixels):
    values = [0.2126 * r + 0.7152 * g + 0.0722 * b for r, g, b in pixels]
    mean = sum(values) / len(values)
    variance = sum((value - mean) ** 2 for value in values) / len(values)
    return mean, variance


def assert_not_flat(directory: Path, path: Path, label: str) -> None:
    if not path.exists():
        fail(directory, f"{label} missing: {path}")
    size, pixels = samples(path)
    if len(pixels) < 16:
        fail(directory, f"{label} has too few pixels")
    mean, variance = luminance_stats(pixels)
    log(directory, f"{label} {size[0]}x{size[1]} mean={mean:.1f} variance={variance:.1f}")
    if variance < 40:
        fail(directory, f"{label} is a single color (variance {variance:.1f})")
    if mean < 22 and variance < 80:
        fail(directory, f"{label} is uniform dark (mean {mean:.1f})")


def assert_pin_colors(directory: Path, path: Path) -> None:
    if not path.exists():
        fail(directory, f"pin screenshot missing: {path}")
    image = Image.open(path).convert("RGB")
    red = blue = yellow = 0
    for y in range(0, image.size[1], 2):
        for x in range(0, image.size[0], 2):
            r, g, b = image.getpixel((x, y))
            if r > 190 and g < 70 and b < 70:
                red += 1
            if b > 170 and r < 70 and g < 100:
                blue += 1
            if r > 190 and g > 160 and b < 90:
                yellow += 1
    log(
        directory,
        f"pin screenshot {image.size[0]}x{image.size[1]} red={red} blue={blue} yellow={yellow}",
    )
    if red < 40 or blue < 40 or yellow < 10:
        fail(directory, "pin screenshot does not show the red/blue/yellow test image")


def os_screenshot(path: Path, platform: str) -> None:
    if platform == "macos":
        subprocess.check_call(["/usr/sbin/screencapture", "-x", str(path)])
        return
    destination = str(path.resolve()).replace("'", "''")
    script = f"""
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
$vs = [System.Windows.Forms.SystemInformation]::VirtualScreen
$bmp = New-Object System.Drawing.Bitmap ([Math]::Max(1, $vs.Width)), ([Math]::Max(1, $vs.Height))
$graphics = [System.Drawing.Graphics]::FromImage($bmp)
$graphics.CopyFromScreen($vs.X, $vs.Y, 0, 0, $bmp.Size)
$bmp.Save('{destination}', [System.Drawing.Imaging.ImageFormat]::Png)
"""
    subprocess.check_call(["powershell", "-NoProfile", "-Command", script])


def has_cjk(text: str) -> bool:
    return any("\u4e00" <= char <= "\u9fff" for char in text)


def check_translation(directory: Path) -> None:
    path = directory / "translate-result.json"
    if not wait_for(directory, "translate-result.json", 40):
        fail(directory, "translation result was not written")
    payload = json.loads(path.read_text(encoding="utf-8"))
    status = payload.get("status")
    log(directory, "translate payload " + json.dumps(payload, ensure_ascii=False))
    if status == "network":
        log(directory, "translate network blocked; not failing the job")
        return
    text = str(payload.get("text") or "")
    if status == "ok" and has_cjk(text):
        log(directory, "translate ok")
        return
    fail(directory, "translation did not return Chinese text")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--dir", required=True)
    parser.add_argument("--platform", choices=["macos", "windows"], required=True)
    args = parser.parse_args()
    directory = Path(args.dir)
    directory.mkdir(parents=True, exist_ok=True)
    log(directory, f"waiting on {args.platform}")
    if not wait_for(directory, "overlay-ready", 300):
        fail(directory, "overlay was not ready in time")
    time.sleep(1)
    overlay = directory / "overlay-screen.png"
    os_screenshot(overlay, args.platform)
    assert_not_flat(directory, directory / "frozen-capture.png", "frozen capture")
    assert_not_flat(directory, overlay, "overlay screen")
    if not wait_for(directory, "pin-ready", 90):
        fail(directory, "pin was not ready in time")
    time.sleep(1)
    pin_shot = directory / "pin-screen.png"
    os_screenshot(pin_shot, args.platform)
    assert_not_flat(directory, pin_shot, "pin screen")
    assert_pin_colors(directory, pin_shot)
    check_translation(directory)
    wait_for(directory, "smoke-done", 40)
    log(directory, "smoke checks passed")


if __name__ == "__main__":
    main()
