#!/usr/bin/env python3
"""Fail if desktop and Android window-backend features leak across targets."""

import re
import subprocess
import sys


def features(package, target):
    tree = subprocess.check_output(
        [
            "cargo", "tree", "--locked", "--target", target,
            "--edges", "features", "--prefix", "none", "--invert", package,
        ],
        text=True,
    )
    selected = set(re.findall(rf'^{package} feature "([^"]+)"', tree, re.MULTILINE))
    print(f"{target}: {package} features: {', '.join(sorted(selected))}", flush=True)
    return selected


def require(selected, expected, forbidden):
    missing = expected - selected
    leaked = forbidden & selected
    if missing or leaked:
        raise SystemExit(f"Feature mismatch: missing={sorted(missing)}, forbidden={sorted(leaked)}")


for target in sys.argv[1:]:
    android = "android" in target
    platform = {"android-native-activity"} if android else {"x11", "wayland"}
    forbidden = {"x11", "wayland"} if android else {"android-native-activity", "android-game-activity"}
    require(features("eframe", target), platform | {"glow", "default_fonts"}, forbidden)
    require(features("winit", target), platform, forbidden)
