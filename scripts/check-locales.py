#!/usr/bin/env python3
"""Report translation keys used in the code but missing from locales/en.yml.

Run: python scripts/check-locales.py   (exit code 1 when keys are missing)
"""
import glob
import pathlib
import re
import sys

import yaml

ROOT = pathlib.Path(__file__).resolve().parent.parent
locale = yaml.safe_load((ROOT / "locales" / "en.yml").read_text(encoding="utf-8"))


def has(key):
    node = locale
    for part in key.split("."):
        if not isinstance(node, dict) or part not in node:
            return False
        node = node[part]
    return True


keys = set()
for path in glob.glob(str(ROOT / "crates" / "**" / "*.rs"), recursive=True):
    text = pathlib.Path(path).read_text(encoding="utf-8")
    for m in re.finditer(r'"((?:[a-z_]+\.)+[a-z_0-9]+)"', text):
        if m.group(1).split(".")[0] in locale:
            keys.add(m.group(1))

missing = sorted(k for k in keys if not has(k))
for k in missing:
    print(f"missing: {k}")
print(f"{len(keys)} keys used, {len(missing)} missing")
sys.exit(1 if missing else 0)
