# Open-issue triage & fix plan (branch plan/moth-triage, base main 97e0bb1)

Updated 2026-10-10 from the agent channel. The two heads `main`/`build` agreed
at `fde0548` then again later; `main` is `1912f98` (Android import-pair fix,
CI green). This plan now doubles as the live assignment map.

## Assignment map (who owns what)
- **Moth (me)**: #22 ✅, #19 ✅ (both landed), #13 ✅ (landed, Android-only).
- **Cherry**: Wave 0 close-out — #67 #66 #49 #38 #57 #37 #35 #7 #25 #14 #8 #16
  #6 #15 #31 (closed with shas in channel). Plus #9/#18 residue (still open).
- **Mycel**: #58 residue (`refresh_from_history()`, no XDG/JNI rescan per tag
  save) + fork of `graft/lathe` (SearchIndex, LRU icon cache). Branch
  `graft/mycel`.
- **Scion07**: #28 (Linux GUI assertion rig in CI).
- **Lathe/Kestrel/Trellis/Ibis**: UX lane / perf lanes; Lathe orphaned (Mycel
  forked it).

## Progress (Moth)
- ✅ **#22** fixed + 2 regression tests (`bd16817`).
- ✅ **#19** already resolved on main (`storage.rs::platform_data_dir` ->
  `internal_data_path()`); no relative `path()` remains. Mark close.
- ✅ **#13** fixed on branch (`279f9db`): `get_app_category` error path now
  clears the pending JNI exception before returning, so discovery no longer
  aborts on API 21-25. This is the exact item Cherry flagged as still-open in
  the channel — kestrel's SDK_INT guard was never merged, so it was live.
  Android-only -> `aarch64-android check` + API-21 device.
- Offered to take the new human-reported UI spacing bug (unclaimed, see below).

## Resolved / closed (do not duplicate)
| # | Title | Closed by | Note |
|---|---|---|---|
| 31 | LICENSE | main | file exists |
| 40/49/66/38/67 | BrowserBack, kbd prefs, full-width rows, settings UI, Android uninstall | ab0dded / Cherry | |
| 37/35/7 | perf / scroll accel | 3174c06, fling.rs, 4a240f0 | see #37 numbers |
| 34/14/8 | icons | c9dfcf9 | |
| 25 | transparent theme | Cherry | alpha rig proves it works |
| 6/15/16 | OSK / search at bottom | Cherry | #9/#18 still open |
| 51 | 600px Wayland clip | 0d79fad (main) | Cherry verified |
| 57 | desktop uninstall | Cherry | residual = confirm prompt (ab0dded); do NOT guard the remaining AppImage delete |
| 28 | CI emulator/GUI tests | Scion07 | scripts/gui-assertions.sh |
| 58 | stale list (residue) | Mycel | refresh_from_history() |

## Still open
- **#9 / #18** — taps vs scroll, buttons not consistently pressable. Cherry left
  these open (no commit to point at). OSK fixes landed for #6/#15/#16 but these
  two remain; likely need the same gesture/egui hit-test work.
- **#13** — was open; now fixed on this branch (pending aarch64-android check).

## New from @human (2026-10-10)
> "Latest build got bug with too much empty space between apps and search.
> Better have no horizontal line and few empty space."

Unclaimed UI bug. Candidate owner: Moth (offered in channel). Fix shape:
remove the separator `Separator`/`hline` between the results list and the
search bar in `src/ui/mod.rs`, and tighten the vertical gap (reduce the
`egui::Spacing` / `vertical`/`min_gap` around the search field). Hard to verify
headlessly; guard with the GUI assertion rig (#28) if it can capture layout.

## Wave 3 — product decisions (DECIDE, @human)
- **#27 INTERNET permission** — recommend remove until web search exists.
- **#24 / #39** phantom providers / settings not all recreated — recommend hide
  unimplemented providers, track each separately.
- **#33 themes**, **#64 localization** — features; after the bug waves.
- **#30 docs** — consolidate root .md files last.

## Rules (carried from channel)
- One dialect per issue: claim in chat before coding.
- Tests must exercise the real function, not re-derive the assertion.
- Receipts per PR: `cargo test`, `clippy -D warnings`, `fmt`, `aarch64-android check`.
- Toolchain tip (Mycel): `rustup` + stable installs in ~90s; this workspace
  started with no cargo too.
