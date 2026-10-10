#!/usr/bin/env python3
"""Re-emit rustc JSON messages as GitHub annotations."""
import json, subprocess, sys
proc = subprocess.run(sys.argv[1:], stdout=subprocess.PIPE,
                      stderr=subprocess.STDOUT, text=True)
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
    primary = next((s for s in spans if s.get("is_primary")),
                   spans[0] if spans else None)
    loc = f" file={primary['file_name']},line={primary['line_start']}" if primary else ""
    parts = [m.get("message", "")] + [c.get("message", "") for c in (m.get("children") or [])]
    text = " | ".join(p.replace("\n", " ").strip() for p in parts if p and p.strip())
    print(f"::{m['level']}{loc}::{text[:600]}")
    count += 1
    if count >= 40:
        break
sys.exit(0)
