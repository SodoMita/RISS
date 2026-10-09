# RISS Launcher (Rust + egui)

A minimalist application launcher inspired by RISS Launcher for Android, built with Rust and egui. Works on Linux and can be adapted for other platforms.

![RISS Launcher](https://img.shields.io/badge/platform-Linux-blue) ![Rust](https://img.shields.io/badge/rust-1.70+-orange) ![License](https://img.shields.io/badge/license-MIT-green)

## Features

✨ **Minimalist Interface** - Clean, distraction-free UI with search bar at the bottom (RISS style)

🔍 **Fuzzy Search** - Find apps quickly with intelligent fuzzy matching

⭐ **Favorites** - Pin apps in a quick-launch bar and manage them from app actions

📊 **Usage Tracking** - Automatically tracks launch counts and supports recent, frequent, or alphabetical ordering

🏷️ **Custom Tags** - Add searchable tags to apps from the tag button or long-press menu

🧮 **Calculator & Web Search** - Calculate expressions or send a query to DuckDuckGo, Google, or Brave

⚙️ **Settings** - Persisted controls for themes, result density, providers, history, favorites, touch gestures, and hidden apps

👆 **Touch-first controls** - Large tap targets, tap-to-launch rows, long-press actions, and configurable swipe/double-tap gestures

⌨️ **Keyboard Navigation** - Full keyboard support for fast operation

🎨 **Dark Theme** - Beautiful dark theme with Catppuccin-inspired colors

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
- Click the star (★/☆) icon next to any app to add/remove it from favorites
- Favorites appear at the top when the search field is empty

#### Tags
- Click the tag icon (🏷️) next to an app to add custom tags
- Tags help you find apps using custom keywords
- Example: Tag Firefox with "web", "internet", "browser"

#### Calculator and web search
- Type mathematical expressions directly in the search bar
- Supported operators: `+`, `-`, `*`, `/`, `^` (power), and parentheses
- Tap a calculator result to copy it
- If an app doesn't match, tap the web-search action to open your selected provider

#### Touch behavior and settings
- Tap the main area of an app row to launch it; tap the star to pin or unpin it
- Tap the tag button to edit tags, or press and hold a result to open its action menu
- Search focus is requested once at launch, not forced every frame, so app and settings buttons remain touchable
- Open **Settings** with the gear button to choose a theme/accent, result density, visible search fields, home view, history sort, favorites bar, app exclusions, and gesture actions
- Swipe and double-tap actions are assigned in Settings and performed on the RISS title area
- Settings are saved separately from history. Use **Advanced** to copy, import, or restore preferences

#### App Discovery
- Automatically scans standard Linux application directories:
  - `/usr/share/applications`
  - `/usr/local/share/applications`
  - `~/.local/share/applications`
- Reads `.desktop` files to discover installed applications
- Click the refresh button (🔄) to rescan for new apps

## Configuration

Launcher preferences are stored in `~/.config/riss-launcher/settings.json` (or `$XDG_CONFIG_HOME/riss-launcher/settings.json`). Launch history and favorites remain in `~/.config/riss-launcher/history.json`.

Preferences include:
- System, light, dark, and AMOLED themes with accent colors
- Home view and result limit, app search fields, calculator, and web provider
- Result size, icons, descriptions, tags, separators, search bar, and favorites bar appearance
- History ranking, pause-history, favorite exclusions, and reset actions
- Long-press, swipe, and double-tap actions; hidden apps; settings import/export

History data contains launch counts, last-launch timestamps, favorite app IDs, and custom tags.

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

### Project Structure

```
riss-launcher/
├── src/
│   ├── main.rs          # Entry point
│   ├── ui.rs            # UI components and main app state
│   ├── app_entry.rs     # App discovery and .desktop file parsing
│   ├── search.rs        # Search engine and fuzzy matching
│   └── history.rs       # History tracking and persistence
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

## KISS-inspired features and platform limits

| Feature area | RISS support |
|--------------|--------------|
| Fuzzy app search, descriptions, categories, and tags | ✓ |
| Tap-to-launch, long-press actions, favorites, and usage history | ✓ |
| Light/dark/system/AMOLED themes, accent colors, and result sizing | ✓ |
| Calculator and configurable web search providers | ✓ |
| Hidden apps, settings import/export, and launcher gestures | ✓ |
| Contacts, call history, icon packs, and Android notification shade | Not available in this cross-platform build |

## Future Enhancements

- [ ] File search integration
- [ ] Command execution (run shell commands)
- [ ] Plugin system
- [ ] Wayland-specific optimizations
- [ ] Native system icon and icon-pack loading
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
