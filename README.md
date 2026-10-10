# RISS Launcher (Rust + egui)

A minimalist application launcher inspired by RISS Launcher for Android, built with Rust and egui. Works on Linux and can be adapted for other platforms.

![RISS Launcher](https://img.shields.io/badge/platform-Linux-blue) ![Rust](https://img.shields.io/badge/rust-1.70+-orange) ![License](https://img.shields.io/badge/license-MIT-green)

## Features

✨ **Minimalist Interface** - Clean, distraction-free UI with search bar at the bottom (RISS style)

🔍 **Fuzzy Search** - Find apps quickly with intelligent fuzzy matching

⭐ **Favorites** - Mark apps as favorites, then long-press the bar to remove, reorder and retag them, or edit the whole bar on one screen

📊 **Usage Tracking** - Automatically tracks launch counts and shows frequently used apps

🏷️ **Custom Tags** - Add custom tags to apps for better organization

🧮 **Calculator** - Built-in calculator for quick math (supports +, -, *, /, ^, parentheses), with one-click copy

🌐 **Providers** - Web search (configurable engine), shell commands, timers with audible alarm, settings search and previous searches — all offered as extra rows under your app matches, just like KISS

⏱️ **Timers** - Type `timer 5m` or `sleep 30s` to start a countdown; activate the row again to cancel

✏️ **Rename & shortcuts** - Long-press any result to rename it, pin it to the numbers 1–9 (type the digit to jump straight to it), copy its name or command, or open its `.desktop` file

💾 **Import / export** - Back up settings and history to a single JSON file and restore them later

👆 **Touch-first controls** - Large tap targets, long-press app actions, swipe gestures, and a KISS-style bottom bar

⚙️ **Complete settings** - Persistent History, Favorites, Appearance, Icons, Gestures, Providers, Exclusions, and Advanced settings matching every KISS preference

⌨️ **Keyboard Navigation** - Full keyboard support for fast operation

🎨 **Themes** - Dark, light, transparent, accent-color, icon, result-size, and layout controls

🖼️ **Wallpaper** - The transparent theme shows the home-screen wallpaper behind the launcher on Android (results, search bar and settings keep solid backgrounds)

## Installation

### Prerequisites

- Rust 1.70 or later
- Linux with X11 or Wayland
- Required system libraries:
  ```bash
  sudo apt-get install -y libx11-dev libgl1-mesa-dev libfontconfig1-dev \
    pkg-config libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev
  ```

### Building from Source

```bash
# Clone or download the repository
cd riss-launcher

# Build the project
cargo build --release

# Run the launcher
./target/release/riss_launcher
```

## Usage

### Basic Operation

1. **Launch the app** - Run `riss_launcher` from terminal or create a desktop shortcut
2. **Type to search** - Start typing to search through installed applications
3. **Navigate results** - Use ↑↓ arrow keys or mouse to select an app
4. **Launch** - Press Enter or click on an app to launch it
5. **Clear search** - Press Escape to clear the search field

### Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `↑` / `↓` | Navigate through results |
| `Enter` | Launch selected app |
| `Esc` | Clear search / Close |
| `Tab` | Cycle through results |

### Features Guide

#### Favorites
- Long-press (or right-click) any app and pick **Add to favorites**
- Favorites sit in the bar above the search field and launch on a single tap
- Long-press a favorite in the bar for the same menu a search row shows (as in KISS): **Open**, **Remove favorite**, **Move left** / **Move right**, **Edit tags**, **Manage favorites…**
- Settings → Favorites → **Manage favorites** edits the whole bar: ◀ ▶ reorder, ✕ remove, 🏷 retag, **Clear all favorites**, and a search box to add more
- Long press a favorite and drag it sideways to reorder the bar, as in KISS: the popup closes once the icon moves and the order is saved when you let go
- **Move left** / **Move right** in the same menu reorder one slot at a time; a plain swipe still scrolls the bar
- Favorites whose app was uninstalled or excluded stay listed there, so stale entries can be deleted instead of being unreachable

#### Tags
- Click the tag icon (🏷️) next to an app to add custom tags
- Tags help you find apps using custom keywords
- Example: Tag Firefox with "web", "internet", "browser"

#### Calculator
- Type mathematical expressions directly in the search bar
- Supported operators: `+`, `-`, `*`, `/`, `^` (power)
- Supports parentheses: `(5+3)*2`
- Example: Type `2^10` to see `= 1024`, then hit **Copy** to grab the answer

#### Providers
- Type almost anything to also get provider rows below the app matches:
  - **Web search** opens the query in your configured search engine
  - **Shell commands** run multi-word queries in `sh -c` (desktop only)
  - **Timers** start from `timer 5m`, `sleep 30s`, `1h` …
  - **Settings search** finds any preference straight from the query bar — activate it to jump to that setting
  - **Previous searches** resurface earlier web searches
- Each provider can be switched off under *Settings → Search providers*

#### Rename & number shortcuts
- Long-press a result to rename it, favorite it, edit tags, reset its rank, hide it, or pin it to a number
- Type `1`–`9` in the search bar to jump to the app pinned to that number
- The context menu also copies the name/command and opens the `.desktop` file

#### App Discovery
- Automatically scans standard Linux application directories:
  - `/usr/share/applications`
  - `/usr/local/share/applications`
  - `~/.local/share/applications`
- Reads `.desktop` files to discover installed applications
- Click the refresh button (🔄) to rescan for new apps

## Configuration

Settings and history are stored as JSON:

```
~/.config/riss-launcher/settings.json   # all KISS-compatible preferences
~/.config/riss-launcher/history.json    # launch counts, timestamps, favorites, tags
```

On Android the same two files live in the app-private directory
(`Context.getFilesDir()`, e.g. `/data/data/com.risslauncher.app/files/`).
Set `RISS_DATA_DIR` to store them somewhere else.

Writes are atomic, and a file that cannot be parsed is moved aside as
`<name>.corrupt-<timestamp>` instead of being overwritten, so data is never
lost silently.

`history.json` contains:
- Launch counts for each app
- Last launch timestamps
- Favorite apps list
- Custom tags

The empty-screen list can be sorted by recency, frequency, `frecent`
(usage that decays with age) or alphabetically via the *History sorting*
setting.

## Screenshots

The launcher features:
- **Empty state**: Shows favorites and frequently used apps
- **Search**: Instant fuzzy search with highlighted matches
- **Calculator**: Quick calculations without leaving the launcher
- **Tags**: Custom organization system

## Customization

### Colors
Colors are defined in `src/ui.rs` in the `Colors` struct. You can customize the theme by modifying these constants:

```rust
struct Colors;
impl Colors {
    const BG: Color32 = Color32::from_rgb(30, 30, 46);
    const TEXT: Color32 = Color32::from_rgb(205, 214, 244);
    const ACCENT: Color32 = Color32::from_rgb(137, 180, 250);
    // ... more colors
}
```

### Window Size
Modify the window size in `src/main.rs`:

```rust
.with_inner_size([420.0, 700.0])  // width, height
.with_min_inner_size([320.0, 400.0])
```

## Development

### Testing & tooling

- `cargo fmt -- --check`, `cargo clippy -- -D warnings`, `cargo test` and an
  Android `cargo check` run in CI on every pull request.
- `bash scripts/gui-smoke-test.sh [xvfb|wayland]` starts the launcher on a
  headless display and screenshots it (CI runs both modes and uploads the
  screenshots as the `smoke-screenshots` artifact). Requires `xvfb` +
  ImageMagick or `cage` + `grim`.
- Building inside a network-restricted sandbox (no crates.io, no rustup, no
  apt)? See [RESTRICTED-ENVIRONMENT.md](RESTRICTED-ENVIRONMENT.md) for the
  verified bootstrap: Rust toolchain from npm, an offline vendored-sources
  test harness, CI-as-compiler debugging, and GUI testing options (small
  Wayland compositors included).

### Project Structure

```
riss-launcher/
├── src/
│   ├── main.rs          # Entry point
│   ├── lib.rs           # Android entry point (cdylib)
│   ├── ui/              # UI components and main app state
│   │   ├── mod.rs       # App state, providers, timer, actions
│   │   ├── results.rs   # Result rows, favorites, context menu
│   │   ├── favorites.rs # Favorites manager: delete, reorder, retag, add
│   │   ├── search_bar.rs# Search bar and keyboard handling
│   │   ├── settings.rs  # Settings screen and backup actions
│   │   ├── colors.rs    # Theme palette
│   │   └── touch.rs     # Gestures
│   ├── app_entry.rs     # App discovery and .desktop file parsing
│   ├── android_app_entry.rs  # Android app discovery and launching via JNI
│   ├── search.rs        # Search engine, fuzzy matching, provider rows
│   ├── providers.rs     # Web search, shell commands, timers, clipboard
│   ├── settings.rs      # KISS-compatible preferences
│   ├── storage.rs       # Atomic, crash-safe persistence for both data files
│   └── history.rs       # History tracking, favorites, tags, renames, ranking
├── Cargo.toml           # Dependencies
└── README.md           # This file
```

### Dependencies

- **eframe/egui** - Immediate mode GUI framework
- **fuzzy-matcher** - Fuzzy string matching
- **freedesktop-desktop-entry** - Parse .desktop files
- **serde/serde_json** - Serialization for history
- **open** - Cross-platform app launching

### Building for Release

```bash
cargo build --release
```

The optimized binary will be in `target/release/riss_launcher`.

## Comparison with RISS Launcher (Android)

| Feature | KISS Android | This Implementation |
|---------|--------------|---------------------|
| Search bar position | Bottom | Bottom ✓ |
| Fuzzy search | Yes | Yes ✓ |
| Favorites | Yes | Yes ✓ |
| Tags | Yes | Yes ✓ |
| Calculator | Yes | Yes ✓ |
| Contact search | Yes | No (Linux-focused) |
| Settings search | Yes | Partial |
| Minimalist UI | Yes | Yes ✓ |
| Usage tracking | Yes | Yes ✓ |

## Future Enhancements

- [ ] Support for web searches
- [ ] File search integration
- [ ] Command execution (run shell commands)
- [ ] Custom themes/skins
- [ ] Plugin system
- [ ] Wayland-specific optimizations
- [ ] Icon loading from system themes
- [ ] Multi-language support

## Troubleshooting

### App doesn't launch
- Check if the app's executable is in your PATH
- Verify the .desktop file has a valid Exec line
- Check terminal output for error messages

### Apps not showing up
- Click the refresh button (🔄) to rescan
- Ensure .desktop files are in standard locations
- Check that apps aren't marked as `NoDisplay=true` or `Hidden=true`

### Font issues
- The app tries to load DejaVu Sans from system fonts
- Falls back to egui's default font if not found
- You can customize font loading in `src/ui.rs`

## License

MIT License - feel free to use and modify as needed.

## Contributing

Contributions are welcome! Feel free to:
- Report bugs
- Suggest features
- Submit pull requests
- Improve documentation

## Credits

- Inspired by [RISS Launcher](https://github.com/Neamar/KISS) for Android
- Built with [egui](https://github.com/emilk/egui) - an immediate mode GUI library
- Uses [fuzzy-matcher](https://github.com/lotabout/fuzzy-matcher) for search

## Support

For issues, questions, or contributions, please open an issue on the project repository.

---

**Made with ❤️ using Rust and egui**
