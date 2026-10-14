#!/usr/bin/env python3
"""Check that every locales/*.yml has exactly the keys of en.yml, the same
%{placeholders} per key, and no empty values. Exit code 1 on any problem."""
import re
import sys
from pathlib import Path

import yaml

LOCALES = Path(__file__).resolve().parent.parent / "locales"
PLACEHOLDER = re.compile(r"%\{(\w+)\}")


def flatten(node, prefix=""):
    out = {}
    for k, v in node.items():
        key = f"{prefix}.{k}" if prefix else str(k)
        if isinstance(v, dict):
            out.update(flatten(v, key))
        else:
            out[key] = v
    return out


def load(path):
    return flatten(yaml.safe_load(path.read_text(encoding="utf-8")))


def main():
    base = load(LOCALES / "en.yml")
    problems = 0
    files = sorted(p for p in LOCALES.glob("*.yml") if p.stem != "en")
    for path in files:
        tr = load(path)
        errs = []
        errs += [f"missing: {k}" for k in base if k not in tr]
        errs += [f"extra: {k}" for k in tr if k not in base]
        for k, v in tr.items():
            if k not in base:
                continue
            if not isinstance(v, type(base[k])) or (isinstance(v, str) and not v.strip()):
                errs.append(f"bad/empty value: {k}")
            elif isinstance(v, str) and set(PLACEHOLDER.findall(v)) != set(PLACEHOLDER.findall(base[k])):
                errs.append(f"placeholder mismatch: {k}")
        status = "OK" if not errs else f"{len(errs)} problem(s)"
        print(f"{path.name:10} {len(tr)}/{len(base)} keys  {status}")
        for e in errs:
            print("   ", e)
        problems += len(errs)
    print(f"\n{len(files)} locales checked against en.yml ({len(base)} keys)")
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
