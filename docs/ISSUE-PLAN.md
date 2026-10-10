# Open-issue triage & fix plan (branch plan/moth-triage, base main 97e0bb1)

Legend: CLOSE = believed fixed on main, needs human verify then close · FIX = work remaining · DECIDE = needs owner call.

## Wave 0 — verify & close (no code, cheap wins)
| # | Title | Evidence on main |
|---|---|---|
| 31 | LICENSE | `LICENSE` exists at repo root |
| 13 | JNI category API 21-25 | SDK_INT guard + exception_clear (kestrel, ab0dded) |
| 40/49/66/38/67 | BrowserBack, kbd prefs, full-width rows, settings UI, Android uninstall | ab0dded |
| 37/35/7 | perf / scroll accel / scrolling | 3174c06 virtualisation, fling.rs, 4a240f0 alloc cut (see #37 numbers) |
| 34/14/8 | icons | IconCache c9dfcf9 — verify real PNG/SVG in bottom bar (#34 specifically SVG) |
| 25 | transparent theme | colors.rs: only "transparent" is see-through — verify on device |
| 6/15/16/10/9/17/18 | OSK, search at bottom, taps vs scroll | fix/osk-reservation-floor merged? verify on device, close duplicates (6≈15/16, 9≈18) |

## Wave 1 — correctness bugs (one owner each, small PRs)
1. **#58 stale app list** — single `refresh_views()` after launch/rank reset/tag edit; regression test that search sees the update.
2. **#57 desktop uninstall deletes .desktop** (package_managers.rs:560) — never delete system files; route to package manager with confirm, or hide action when owner unknown. Depends on fix/uninstall-escalation branch — merge or supersede.
3. **#22 history omits most recent** — sort by recency first, then cap; test with 21 apps.
4. **#19 relative paths on Android** — resolve via `internalDataPath`; test path is absolute.
5. **#51 600px Wayland clip** — clamp DEFAULT_INNER_SIZE to monitor height minus decorations (fix/wayland-app-id lane nearby).

## Wave 2 — Android platform
6. **#20 package change listener** — BroadcastReceiver (PACKAGE_ADDED/REMOVED/REPLACED) or cheap rescan on resume.
7. **#28 CI emulator smoke test** — after Wave 1 so it guards real fixes.

## Wave 3 — product decisions (DECIDE)
- **#27 INTERNET permission** — recommend: remove until web search exists (KISS parity, privacy).
- **#24 phantom providers** / **#39 settings not all recreated** — recommend hide unimplemented providers now, track each as its own issue.
- **#33 hardcoded themes**, **#64 localization** — features; after Waves 1-2. Localization: fluent or simple key tables.
- **#30 docs** — consolidate ~8 root md files into README + docs/ at the end.

## Rules (carried from channel)
- One dialect per issue: claim in chat before coding.
- Tests must exercise the real function, not re-derive the assertion.
- Receipts per PR: cargo test, clippy -D warnings, fmt, aarch64-android check.
