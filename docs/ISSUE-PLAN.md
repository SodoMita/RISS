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

## Progress (Moth / archive)
- ✅ **#22** fixed + 2 regression tests (`bd16817`), and verified: `cargo
  check --lib` green on stable host, `cargo test --lib search::` 7/7 (incl.
  `default_apps_never_drop_a_just_launched_app`,
  `default_apps_pin_favorites_above_recency`).
- ↩️ **#13** duplicate WITHDRAWN: Cherry already fixed it on main
  (`a7266ea`, identical `exception_clear` in `get_app_category`'s error
  path). I reverted my `279f9db` on this branch so we don't carry a second
  dialect. Verify on main.
- ✅ **#19** already resolved on main (`storage.rs::platform_data_dir` ->
  `internal_data_path()`); no relative `path()` remains. Mark close.
- ✅ **@human UI bug** (empty space + horizontal line between results and
  search bar) FIXED on branch (`14d1727`): removed the search bar border
  stroke and tightened the panel/central gaps. Compile-unverified (sandbox
  wiped the toolchain cache); visual regression belongs to the #28 rig.
- 🔎 **#58** resolved by Mycel (`refresh_from_history()`); #28 CI rig by
  Scion07; #57 closed by Cherry.

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
  two remain; the other Moth ("lamp") took #18 in PR #86 (headless kittest of
  the real RissApp). Likely the same gesture/egui hit-test work.
- **#13** — fixed on main by Cherry (`a7266ea`); my duplicate on this branch
  was reverted. Verify on main.

## New from @human (2026-10-10)
> "Latest build got bug with too much empty space between apps and search.
> Better have no horizontal line and few empty space."

FIXED on plan/moth-triage (14d1727, Moth). There was no literal Separator --
the "line" was the search bar's 1px p.border stroke (its top edge read as a
divider) and the "empty space" was the bottom panel's +20.0 slack height plus
same(10) margins. Fix:
- src/ui/search_bar.rs: border stroke -> Color32::TRANSPARENT (no divider line;
  bar keeps its fill).
- src/ui/mod.rs: launcher-search panel exact_height +20.0 -> +2.0, inner margin
  same(10) -> symmetric(10,3); CentralPanel bottom padding removed so results
  meet the bar.
Compile-unverified (sandbox wiped the toolchain cache); covered by the GUI
assertion rig (#28) for visual regression.

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
