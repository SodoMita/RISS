#!/usr/bin/env python3
"""Run a cargo command and re-emit its rustc JSON messages as GitHub
annotations so the output is readable through the checks API."""
import json
import subprocess
import sys

proc = subprocess.run(
    sys.argv[1:], stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True
)
count = 0
for line in proc.stdout.splitlines():
    try:
        msg = json.loads(line)
    except json.JSONDecodeError:
        continue
    if msg.get("reason") != "compiler-message":
        continue
    m = msg["message"]
    if m.get("level") not in ("error", "warning"):
        continue
    spans = m.get("spans") or []
    primary = next(
        (s for s in spans if s.get("is_primary")), spans[0] if spans else None
    )
    loc = ""
    if primary:
        loc = f" file={primary['file_name']},line={primary['line_start']}"
    parts = [m.get("message", "")]
    for child in m.get("children") or []:
        ctext = child.get("message", "")
        if ctext:
            parts.append(ctext)
    if primary:
        for t in (primary.get("text") or [])[:3]:
            line_text = (t.get("text") or "").strip()
            if line_text:
                parts.append(line_text)
    text = " | ".join(
        p.replace("\n", " ").strip() for p in parts if p and p.strip()
    )
    print(f"::{m['level']}{loc}::{text[:600]}")
    count += 1
    if count >= 40:
        break
print(f"reported {count} messages (exit {proc.returncode})")
sys.exit(0)
