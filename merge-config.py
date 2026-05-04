#!/usr/bin/env python3
"""
Merge all config/*.toml files into /zeroclaw-data/.zeroclaw/config.toml.

- Arrays: replaced (overlay wins), not concatenated.
- Tables: deep-merged recursively.
- Merge order: alphabetical by filename.

Input:  /tmp/config.d/*.toml  (from COPY config/ in Dockerfile)
Output: /zeroclaw-data/.zeroclaw/config.toml
"""
import toml
from pathlib import Path
import sys


def deep_merge(base: dict, overlay: dict) -> None:
    for k, v in overlay.items():
        if k in base and isinstance(base[k], dict) and isinstance(v, dict):
            deep_merge(base[k], v)
        else:
            base[k] = v


def main():
    config_dir = Path("/tmp/config.d")
    output_path = Path("/zeroclaw-data/.zeroclaw/config.toml")

    merged = {}
    for fpath in sorted(config_dir.glob("*.toml")):
        data = toml.load(fpath)
        deep_merge(merged, data)
        print(f"  Merged: {fpath.name}")

    output_path.parent.mkdir(parents=True, exist_ok=True)
    with open(output_path, "w") as f:
        toml.dump(merged, f)

    print(f"Written: {output_path}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
