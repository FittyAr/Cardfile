#!/usr/bin/env python3
"""Report keys used in code but missing from English; --all checks translations too.

Run: python scripts/check-locales.py [--all] (exit code 1 on validation errors)
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


def flatten(node, prefix=""):
    result = {}
    for key, value in node.items():
        name = f"{prefix}{key}"
        if isinstance(value, dict):
            result.update(flatten(value, name + "."))
        elif isinstance(value, str):
            result[name] = value
    return result


errors = len(missing)
if "--all" in sys.argv[1:]:
    reference = flatten(locale)
    for path in sorted((ROOT / "locales").glob("*.yml")):
        translated = flatten(yaml.safe_load(path.read_text(encoding="utf-8")))
        problems = []
        for key, value in reference.items():
            if key not in translated or not translated[key].strip():
                problems.append(f"missing or empty: {key}")
            elif sorted(re.findall(r"%\{[^}]+\}", value)) != sorted(
                re.findall(r"%\{[^}]+\}", translated[key])
            ):
                problems.append(f"placeholder mismatch: {key}")
        for problem in problems:
            print(f"{path.name}: {problem}")
        errors += len(problems)
        print(f"{path.name}: {len(reference)} messages, {len(problems)} errors")
sys.exit(1 if errors else 0)
