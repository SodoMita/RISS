# Why KISS scrolls faster than RISS, even in Java

Compared with [Neamar/KISS](https://github.com/Neamar/KISS). The human's report is scroll latency, not search ranking. Ranking (`#21`) and the icon decode budget already landed on main (`c9dfcf9`). The remaining stall is the one KISS never had.

## What the original does

`RecordAdapter.getView` is called by Android `ListView` for the children on screen, with a recycled `convertView`. A flick of a 400-app drawer builds about a dozen rows. `IconsHandler` memoizes the drawable and writes a PNG cache; it does not decode the drawer in the frame the finger moves.

`AbsListView` / `OverScroller` accumulates fling velocity. A second swipe adds. The cap is thousands of pixels per second. egui's `ScrollArea` replaces velocity on pointer-up and damps it at 1000 px/s², and a wheel line is a fixed 40px (`#35`).

A launch does not rescan packages. `AppProvider` reloads from `LauncherApps.Callback`. The click updates the pojo and the history table.

Search, for the record, is also off the UI thread (`Searcher` on `SEARCH_THREAD`), against names folded once (`StringNormalizer`, ASCII skips `Normalizer.normalize`), into a `PriorityQueue` capped at the display limit. That is why a keystroke is cheap. It is not why a flick is cheap. This branch does not replace the ranking `#68` just landed.

## What this branch changes

`graft/ibis`, based on `c9dfcf9`. Not `main`, not `build`.

- The result list uses `ScrollArea::show_rows`. Only the visible pitch is built. The old path cloned every `SearchResult` and called `show_result` for all of them on every frame, so a flick paid for widgets the user could not see. That is the latency IconCache cannot remove: the decode budget was already there, the widget build was not.
- Wheel input on that list is a fling (`src/ui/fling.rs`). Successive ticks add, opposite direction replaces, cap 9000 px/s, friction 380 px/s². Drag stays with egui so touch scrolling is not reimplemented. `line_scroll_speed` elsewhere is 96.
- Launch, favorite toggle, reset rank, and clear-history stamp both `all_apps` and `apps` via `reapply_usage()` and do not call `discover_apps()`. Uninstall, exclusion, rename, and icon changes still reload, because those change the discovered set.

Arrow keys scroll the highlighted row into the virtual window, which a full layout used to do by accident.
