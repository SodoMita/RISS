# Porting rules

RISS is a **port of KISS**, not a launcher inspired by it. Everything in here
exists to make agents and humans *translate* upstream behaviour instead of
inventing a plausible-looking replacement.

If you read one thing: **a module may only claim parity if someone opened the
upstream file and matched it.** Everything else says so out loud.

## Frozen upstream

- Upstream: <https://github.com/Neamar/KISS> (GPL-3.0)
- Pin the exact ref you ported against in your commit message. Do not describe
  a behaviour from memory, from this file, or from a chat message — open the
  Java.
- License: GPL-3.0-or-later. A direct port cannot be MIT.

## The three states a module can be in

Every ported module states its own status in its doc header:

| State | Means | May be cited as parity |
|---|---|---|
| **faithful port** | Someone opened the upstream file and matched it line by line | yes |
| **modeled** | Shape/structure follows KISS, details not line-verified | no |
| **approximation** | Behaviour invented to fill a gap | no, and say so in the code |

Downgrading your own module is a normal, good commit. Silently upgrading one
is the failure this file exists to prevent.

## Current status

| Module | State | Upstream |
|---|---|---|
| `src/kiss/normalizer.rs` | faithful port | `normalizer/StringNormalizer.java` |
| `src/kiss/comparator.rs` | faithful port | `pojo/{Name,ReversedName,Relevance}Comparator.java` |
| `src/kiss/query_boost.rs` | faithful port | `searcher/{Searcher,QuerySearcher}.java`, `db/HistoryMode.java` |
| `src/search.rs` | approximation | ranking is `fuzzy-matcher` + hand-tuned constants, not KISS's relevance |
| `src/history.rs` | modeled | `db/DBHelper.java` history modes |
| `src/providers.rs` | modeled | `dataprovider/*` |

### Known gaps, written down rather than guessed at

- `Pojo.userHandle` (work/private profiles) is not modeled, so
  `NameComparator`'s third comparison step is a no-op.
- `DBHelper`'s history `relevance` values are not ported.
  `query_boost::adjusted_relevance` takes `knownIds` as given, exactly as
  `QuerySearcher.addResults` does. Wiring a real history store in is what makes
  the query-keyed boost live.
- `StringNormalizer`'s position map (used upstream to highlight matched spans)
  is not ported, because nothing in RISS consumes it yet.
- `search.rs` folds with `to_lowercase()`, so accents and dashes are kept and
  "cafe" does not find "Café". `src/kiss/normalizer.rs` is the fix; using it
  per keystroke would regress issue #37, so the corpus must be normalized once
  at load (see "Performance rules").

## Non-negotiables

- **Keep egui.** Same UI on Linux and Android. "Deeper Android integration"
  means a Kotlin/Java host for `LauncherApps`, contacts, notifications,
  shortcuts and IME — not XML layouts and not a second renderer.
- **Gate on capability, never on an OS name.** `cfg!(target_os = "android")`
  standing in for "can this window have alpha" is what shipped the Linux
  wallpaper opaque for a release. Ask the platform what it can do.
- **One dialect per decision.** If a fix already exists on a branch, take it or
  supersede it explicitly. Two copies of one feature is the bug this repo keeps
  re-learning.
- Do not add features KISS does not have at the pinned ref.
- Do not remove Linux support.

## Performance rules (these are load-bearing, see #37)

KISS is fast because of its data flow, not because Java is fast:

1. **Normalize once per app, at load.** Store the result; never normalize a
   whole corpus per keystroke. Only the query is normalized per keystroke.
2. **Bound while collecting.** `Searcher` keeps a `PriorityQueue` capped at the
   display count; do not build then truncate.
3. **Do not clone candidates before truncation.** Rank over borrowed rows.
4. **Reload a provider only on package events**, never after a launch,
   favourite toggle, rename or tag edit.
5. **Decode icons off the render thread** with a per-frame budget.

A change that reintroduces per-keystroke corpus work is a regression even if
every test passes. `src/tests/alloc_count.rs` exists to catch exactly this.

## What a good slice looks like

One vertical slice per PR, and in the description:

1. the upstream files you opened (with the ref);
2. the RISS files you touched;
3. the behaviours you matched, as a list;
4. what you deliberately did **not** do;
5. receipts.

## Receipts

"It compiles" and "the tests are green" are not receipts if the tests never ran
the thing you changed. State what you measured:

```
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo check --target aarch64-linux-android --all-targets
```

For anything visual, `scripts/gui-assertions.sh` measures pixels under headless
sway. A GUI test that silently skips because no compositor started is worse
than no test — it must fail loudly instead.

If you could not measure something, say which part is unverified. A named gap
is cheap; a false claim of parity costs the next person a day.
