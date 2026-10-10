# RISS Launcher Development Roadmap

*Proposed milestones, not announced release dates. Last reviewed: October 2026.*

## Vision

Make RISS a dependable, minimal Android home launcher while preserving its Linux launcher. Prioritize everyday usability over adding more settings.

**Core principle**: RISS and KISS are comparable as text-first, search-driven launchers. The gap isn't Rust vs Java or egui vs Android Views—it's that several behaviors behind the text interface are missing or incorrect. "KISS-compatible" should describe **working behavior**, not matching preference names.

## Current State (v0.1.0)

- ✅ Rust/egui foundation with Android and Linux support
- ✅ Android app discovery via JNI
- ✅ Basic search, favorites, and settings UI
- ✅ Touch and settings update merged (PR #2)
- ❌ 8 open issues blocking Android daily use
- ❌ Feature parity gaps with KISS
- ❌ Quality and documentation gaps

## Milestones

### v0.2 — Make Android Usable Every Day

**Goal**: Fix blockers preventing daily use on Android devices.

**Issues**: [#13](https://github.com/SodoMita/RISS/issues/13), [#14](https://github.com/SodoMita/RISS/issues/14), [#15](https://github.com/SodoMita/RISS/issues/15), [#16](https://github.com/SodoMita/RISS/issues/16), [#17](https://github.com/SodoMita/RISS/issues/17), [#18](https://github.com/SodoMita/RISS/issues/18), [#19](https://github.com/SodoMita/RISS/issues/19), [#20](https://github.com/SodoMita/RISS/issues/20)

**Critical fixes**:
1. **JNI exception bug** (#13): `get_app_category()` crashes on Android 5-7
2. **App icons** (#14): Load real package icons instead of initials
3. **Keyboard input** (#15): On-screen keyboard doesn't update search reliably
4. **Search bar visibility** (#16): Search bar hidden behind keyboard
5. **Touch scrolling** (#17): Swiping to scroll accidentally launches apps
6. **Touch targets** (#18): Buttons don't consistently receive taps
7. **Data persistence** (#19): History/settings use relative paths
8. **Package changes** (#20): No listener for app install/remove events

**Exit criteria**: On physical phones and emulators (Android 5.0, 7.0, 11+), a user can choose RISS as Home, find and launch installed apps with real icons, use the on-screen keyboard, scroll through results, and restart without losing data.

### v0.3 — Make Features Match Their Settings

**Goal**: Audit every visible setting and ensure it has working behavior or remove it.

**Issues**: [#21](https://github.com/SodoMita/RISS/issues/21), [#22](https://github.com/SodoMita/RISS/issues/22), [#23](https://github.com/SodoMita/RISS/issues/23), [#24](https://github.com/SodoMita/RISS/issues/24), [#25](https://github.com/SodoMita/RISS/issues/25), [#26](https://github.com/SodoMita/RISS/issues/26), [#27](https://github.com/SodoMita/RISS/issues/27)

**Key work**:
1. **Search ranking** (#21): All match types learn from user behavior
2. **History sorting** (#22): Recently-used view shows most recent first
3. **Calculator parser** (#23): Validate full input consumption
4. **Provider audit** (#24): Implement providers OR remove their toggles
5. **Theme transparency** (#25): Make transparent actually transparent OR rename
6. **Uninstall action** (#26): Add uninstall to app menu
7. **INTERNET permission** (#27): Decide on web search or remove permission

**Decision needed**: Should RISS implement KISS-style providers (contacts, shortcuts, web search) or remain app-only?

### v0.4 — Beta and Release Readiness

**Goal**: Quality infrastructure and documentation for public release.

**Issues**: [#28](https://github.com/SodoMita/RISS/issues/28), [#29](https://github.com/SodoMita/RISS/issues/29), [#30](https://github.com/SodoMita/RISS/issues/30), [#31](https://github.com/SodoMita/RISS/issues/31)

**Key work**:
1. **CI expansion** (#28): Android emulator and Linux integration tests
2. **Code modularization** (#29): Split ui.rs into logical modules
3. **Documentation** (#30): Update and consolidate all docs
4. **License** (#31): Add LICENSE file

### v1.0 — Stable Release

**Goal**: Production-ready launcher suitable for daily use.

**Deliverables**: Versioned signed APKs, Linux binaries, upgrade notes, Play Store compliance review, recovery path for default Home users.

**After v1.0**: Consider file search, additional languages, custom themes, and Wayland improvements based on user demand.

## Priority Rationale

**v0.2 before v0.3**: Usability before features. A launcher that crashes on Android 5-7 or has broken keyboard input is not usable regardless of settings.

**v0.3 before v0.4**: Behavior before polish. Settings that don't work are user-facing bugs. CI and refactoring don't fix user-facing issues.

**v0.4 before v1.0**: Quality before release. Automated testing, modular code, and documentation are prerequisites for sustainable releases.

## Testing Strategy

- **v0.2**: Physical Android 5.0, 7.0, 11+ devices and emulators
- **v0.3**: Unit tests for search ranking, calculator, settings persistence
- **v0.4**: CI with Android emulator smoke tests and Linux X11/Wayland regression
- **v1.0**: Signed APK installation, upgrade paths, 30-day daily use testing

## Open Questions

1. **Provider scope**: Implement KISS-style providers or remain app-only?
2. ~~**Theme transparency**: Show wallpaper or rename the theme?~~ Decided:
   the transparent theme shows the home-screen wallpaper on Android (issue
   #32, see [ANDROID.md](./ANDROID.md#wallpaper)). Desktop windows stay solid
   for now — a transparent window there needs a compositing window manager.
3. **Play Store**: Is QUERY_ALL_PACKAGES acceptable?

## Contributing

- [v0.2 issues](https://github.com/SodoMita/RISS/issues?q=is%3Aissue+is%3Aopen+label%3Av0.2) — Android usability blockers
- [v0.3 issues](https://github.com/SodoMita/RISS/issues?q=is%3Aissue+is%3Aopen+label%3Av0.3) — Feature parity
- [v0.4 issues](https://github.com/SodoMita/RISS/issues?q=is%3Aissue+is%3Aopen+label%3Av0.4) — Quality and docs
