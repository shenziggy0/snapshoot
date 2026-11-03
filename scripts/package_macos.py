#!/usr/bin/env python3
"""Seal an unsigned app and include readable installation help in the DMG.

Run after `pnpm tauri build --target <target> --bundles app`.
This uses ad-hoc signing; it does not claim Developer ID trust or notarization.
"""
import argparse
from pathlib import Path
import plistlib
import subprocess
import tempfile


def run(*args):
    subprocess.run([str(arg) for arg in args], check=True)


def package(app: Path, output_dir: Path, arch: str):
    app = app.resolve(strict=True)
    with (app / "Contents/Info.plist").open("rb") as source:
        info = plistlib.load(source)
    if info["CFBundleIdentifier"] != "top.shenziggy.snapshoot":
        raise ValueError("Refusing to package an unexpected application")
    version = info["CFBundleShortVersionString"]
    if not all(c.isdigit() or c in ".-+abcdefghijklmnopqrstuvwxyz" for c in version):
        raise ValueError("Unexpected version format")
    output_dir.mkdir(parents=True, exist_ok=True)
    output = output_dir.resolve() / f"Snapshoot_{version}_{arch}.dmg"
    help_file = Path(__file__).resolve().parent.parent / "docs/install/macOS安装说明.txt"
    with tempfile.TemporaryDirectory(prefix="snapshoot-dmg-") as temp:
        stage = Path(temp) / "contents"
        stage.mkdir()
        staged_app = stage / "Snapshoot.app"
        run("/usr/bin/ditto", app, staged_app)
        # OCR helpers are standalone Mach-O resources. Sign inside-out before
        # sealing the outer bundle; don't use --deep to hide nested failures.
        for resource in sorted((staged_app / "Contents/Resources").rglob("*")):
            if resource.is_file() and not resource.is_symlink():
                kind = subprocess.check_output(["/usr/bin/file", "-b", str(resource)], text=True)
                if "Mach-O" in kind:
                    run("/usr/bin/codesign", "--force", "--sign", "-", resource)
        run("/usr/bin/codesign", "--force", "--sign", "-", "--identifier",
            info["CFBundleIdentifier"], staged_app)
        run("/usr/bin/codesign", "--verify", "--deep", "--strict", staged_app)
        run("/usr/bin/ditto", help_file, stage / "安装前请阅读 - Read Me.txt")
        (stage / "Applications").symlink_to("/Applications")
        # Generate to a temporary file first; don't replace a good artifact if
        # hdiutil fails. The destination is on the same filesystem for rename.
        with tempfile.TemporaryDirectory(prefix=".dmg-", dir=output_dir) as scratch:
            image = Path(scratch) / output.name
            run("/usr/bin/hdiutil", "create", "-volname", "Snapshoot",
                "-srcfolder", stage, "-format", "UDZO", image)
            run("/usr/bin/hdiutil", "verify", image)
            image.replace(output)
    print(output)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--app", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--arch", choices=["aarch64", "x64"], required=True)
    args = parser.parse_args()
    package(args.app, args.output_dir, args.arch)
