# RISS Launcher Development Setup

This document summarizes the development infrastructure that has been set up for the RISS Launcher project.

## GitHub Issues

All known issues and feature requests have been documented as GitHub issues with appropriate labels and milestones:

### v0.2 - Android Usability (8 issues)
- #13: JNI exception not cleared in get_app_category on API 21-25
- #14: App icons show initials instead of real Android package icons
- #15: On-screen keyboard doesn't update search results reliably
- #16: Search bar hidden behind on-screen keyboard
- #17: Touch scrolling accidentally launches apps
- #18: Buttons and touch targets don't consistently receive taps
- #19: History and settings use relative file paths on Android
- #20: Add package change listener for app discovery

### v0.3 - Feature Parity (7 issues)
- #21: Search ranking only learns from fuzzy matches
- #22: Recently-used history can omit the most recent app
- #23: Calculator parser doesn't validate full input consumption
- #24: Settings screen advertises non-existent providers
- #25: Transparent theme paints opaque background
- #26: Missing uninstall action in app menu
- #27: INTERNET permission requested but web search not implemented

### v0.4 - Beta Readiness (4 issues)
- #28: Add Android emulator and Linux integration tests to CI
- #29: Modularize UI code
- #30: Update and consolidate documentation
- #31: Add LICENSE file

## GitHub Milestones

Four milestones have been created to track progress:
- **v0.2 - Android Usability**: Fix blockers preventing daily use on Android
- **v0.3 - Feature Parity**: Make features match their settings
- **v0.4 - Beta Readiness**: Quality infrastructure and documentation
- **v1.0 - Stable Release**: Production-ready launcher

## GitHub Labels

Custom labels have been created for better issue organization:
- **v0.2**, **v0.3**, **v0.4**: Milestone labels
- **android**: Android platform specific issues
- **linux**: Linux platform specific issues
- **core**: Cross-platform core functionality

## Documentation

### ROADMAP.md
A comprehensive development roadmap has been added to the repository root. It includes:
- Vision and core principles
- Current state assessment
- Detailed milestone descriptions with issue links
- Priority rationale
- Testing strategy
- Open questions

### Issue Documentation
Each issue includes:
- Clear problem description
- Code examples showing the bug (where applicable)
- Expected behavior
- Implementation approach (for enhancements)
- Acceptance criteria

## Next Steps

### Immediate (v0.2)
1. Fix the JNI exception bug (#13) - critical for Android 5-7 compatibility
2. Implement real app icon loading (#14)
3. Fix keyboard and touch input issues (#15-18)
4. Implement proper data persistence (#19)
5. Add package change listener (#20)

### Testing
- Set up Android emulator testing in CI (#28)
- Test on physical Android 5.0, 7.0, and 11+ devices
- Validate fixes against acceptance criteria in each issue

### Decision Points
Before v0.3, the team needs to decide:
1. Should RISS implement KISS-style providers (contacts, shortcuts, web search)?
2. Should the "transparent" theme show the wallpaper or be renamed?
3. Is the INTERNET permission justified for Play Store distribution?

## Contributing

To contribute to RISS Launcher:
1. Browse issues by milestone: [v0.2](https://github.com/SodoMita/RISS/milestone/1), [v0.3](https://github.com/SodoMita/RISS/milestone/2), [v0.4](https://github.com/SodoMita/RISS/milestone/3)
2. Look for issues labeled `good first issue` for beginner-friendly tasks
3. Check the acceptance criteria in each issue before starting work
4. Follow the testing strategy for your milestone

## References

- [ROADMAP.md](./ROADMAP.md) - Detailed development roadmap
- [README.md](./README.md) - Project overview and build instructions
- [KISS Launcher](https://github.com/Neamar/KISS) - Original inspiration
