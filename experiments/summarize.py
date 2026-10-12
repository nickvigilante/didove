#!/usr/bin/env python3
"""Summarise a hyperfine JSON export as a Markdown table (wall clock in ms)."""
import json
import sys

for path in sys.argv[1:]:
    data = json.load(open(path))
    print(f"\n`{path}`\n")
    print("| command | mean ms | sd | median ms | min ms | instructions |")
    print("| --- | ---: | ---: | ---: | ---: | ---: |")
    for r in data["results"]:
        if "summary" in r:  # hyperfine >= 1.20 schema
            s = r["summary"]
            w = s["time_wall_clock"]
            ins = s.get("instructions", {}).get("median")
        else:  # older schema: flat fields, no instruction counts
            w, ins = r, None
        ins_s = f"{ins / 1e6:.1f} M" if ins else "-"
        print(
            f"| {r.get('name', r['command'])} | {w['mean'] * 1e3:.2f} | {w['stddev'] * 1e3:.2f} "
            f"| {w['median'] * 1e3:.2f} | {w['min'] * 1e3:.2f} | {ins_s} |"
        )
