# RISS Launcher (Rust + egui)

A minimalist application launcher for Linux and Android, inspired by
[KISS Launcher](https://github.com/Neamar/KISS) and rebuilt with Rust and
[egui](https://github.com/emilk/egui). The whole feature set of KISS — the touch
behaviour, the favourites bar, the tags, the providers and the complete settings
tree — is implemented here on top of an immediate mode toolkit.

![RISS Launcher](https://img.shields.io/badge/platform-Linux%20%2B%20Android-blue) ![Rust](https://img.shields.io/badge/rust-1.70+-orange) ![License](https://img.shields.io/badge/license-MIT-green)

## Features

✨ **KISS layout** - info bar, result list, favourites bar and search bar, every
one of them movable, resizable or hidden from the settings

👆 **Touch first** - tap to launch, long press (or right click) for the context
menu, press animations, comfortable touch targets, configurable long press delay

🔍 **Fuzzy search** - exact / prefix / fuzzy / tag / category matching, a
"min match precision" slider and the optional legacy matcher

⭐ **Favourites bar** - pinned applications, number shortcuts, tags applied to
favourites automatically, single tap or two step launching

📊 **Usage tracking** - launch counts, recency, three ranking modes
(usage count, recency, frecent) and a freezable history

🏷️ **Tags** - per application tags, a tag menu, "show untagged" and tag sorting

🧮 **Providers** - web search, shell commands, timers, calculator, settings
search, excluded apps, previous searches — each one can be disabled

⚙️ **Complete settings** - eleven sections mirroring the KISS preference tree,
searchable both from the settings screen and from the query bar

💾 **Import / export** - everything is plain JSON under
`~/.config/riss-launcher/`

🎨 **Themes** - dark, light and solarized palettes, automatic night mode and a
configurable accent colour

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
git clone <this repository>
cd RISS
cargo build --release
./target/release/riss_launcher
```

`./run.sh` builds and starts the launcher, `./install.sh` installs the binary
and the `riss-launcher.desktop` entry into `~/.local/bin` and
`~/.local/share/applications`.

### Android

```bash
cargo install cargo-ndk
cargo ndk -t arm64-v8a -o target/android/lib/arm64-v8a build --release
```

The Android build uses the very same UI: the launcher lists the installed
packages through JNI and launches them with the package manager.

## Usage

### Basic operation

1. Type to search — applications, providers, settings and special lists all
   answer the same query
2. `↑` / `↓` or a tap selects, `Enter` or a second tap launches
3. Long press (or right click) a result for the context menu
4. `Esc` walks back: menu, editor, dialog, settings, special view, tags, query,
   and finally hides the window

### Keyboard shortcuts

| Key | Action |
|-----|--------|
| `↑` / `↓` | Move the selection |
| `Tab` / `Shift+Tab` | Cycle through the results |
| `Home` / `End`, `PageUp` / `PageDown` | Jump in the list |
| `Enter` | Launch, or confirm the open dialog |
| `Esc` | Back / clear / hide |
| `1` – `9` | Launch the n-th result when the query is empty |
| `F5` | Rescan the installed applications |
| `Ctrl+Alt` + arrows / `L` | Configurable gestures (history, all apps, settings, menu…) |

### Touch and buttons

The interaction model follows KISS as closely as a desktop toolkit allows:

- **Tap** activates a result. With *prevent fast launch* or *double click
  launches* the first tap only selects, exactly like KISS.
- **Long press** (200–1500 ms, configurable) opens the context menu; a right
  click does the same on a mouse.
- **Press feedback** animates every row, favourite and button while it is held.
- Row actions (favourite, tags) are laid out outside the clickable area of the
  row, and rows use drag sensing so scrolling never launches anything.
- Every overlay (context menu, tag editor, rename editor, shortcut pad) closes
  when the pointer touches outside of it.

### Context menu

Long press a result to get the same entries as KISS: open, add/remove from
favourites, edit tags, rename, clear usage data, stop tracking usage, exclude /
restore the application, copy the name or the exec line, open the `.desktop`
file, and pin the application to a number (`1`–`9`).

### Providers

| Provider | Example | Result |
|----------|---------|--------|
| Applications | `fire` | Launch the matching app |
| Settings | `display app names` | Jump to the setting |
| Web | `rust egui` | Open the search engine (`Enter`, custom providers supported) |
| Exec | `ls -la` | Run a shell command |
| Timer | `sleep 5m` | Start a countdown |
| Calculator | `(5+3)*2` | Copy the answer |
| Special views | `history`, `all apps` | Switch the list |

Every provider can be switched off in the settings, and the provider results are
sorted by score so the best match always ends up on top.

## Configuration

Everything lives in two JSON files:

```
~/.config/riss-launcher/settings.json   # every option of the settings screen
~/.config/riss-launcher/history.json    # usage, favourites, tags, exclusions…
~/.config/riss-launcher/riss-backup.json # written by Import / Export
```

Both files are optional: a missing or partial file simply falls back to the
defaults, so upgrading never loses your data.

### Settings sections

| Section | Highlights |
|---------|-----------|
| Touch & buttons | long press menu + delay, prevent fast launch, double click, tap timeout, press feedback, minimum touch height, number keys, gestures |
| Interface | theme, night mode, main colour, rounded list/bars, separators, margins, font scale, fullscreen, portrait, window size |
| Results | result size, adaptive results / icons / columns, separators, app names, sub icons, tags, launch counters, row actions, grid position |
| Search bar | position, info bar position, large bar, hidden hint, keyboard hints, transparency, swapped buttons |
| Favorites | bar on/off, large bar, transparency, position, capacity, exclusion from apps/history, favourite tags, reset |
| History | ranking mode, list length, tracking, freeze, search through history, reset |
| Tags | visibility, tags menu, show untagged, tagged sort, reset |
| Search providers | per provider switches, default provider, custom providers, min match precision, legacy matcher |
| Excluded apps | list, restore, reset |
| Import / export | backup and restore |
| Advanced | reload applications, reset shortcuts, reset everything, about |

## Project structure

```
RISS/
├── src/
│   ├── main.rs            # Desktop entry point
│   ├── lib.rs             # Android entry point (JNI)
│   ├── app_entry.rs       # .desktop discovery, parsing and launching
│   ├── android_app_entry.rs
│   ├── settings.rs        # Settings model, defaults, catalog, persistence
│   ├── history.rs         # Usage, favourites, tags, exclusions, shortcuts
│   ├── search.rs          # Search engine, match types, provider results
│   ├── providers.rs       # Web search, exec, clipboard, timers, calculator
│   ├── theme.rs           # Dark / light / solarized palettes
│   └── ui/
│       ├── mod.rs         # Application state and screens
│       ├── results.rs     # Result list, adaptive grid, favourites bar
│       ├── settings_view.rs # The settings screen
│       └── widgets.rs     # Hand drawn icons and reusable widgets
├── Cargo.toml
└── README.md
```

### Dependencies

- **eframe / egui 0.32** - immediate mode GUI (glow, x11, wayland, Android)
- **fuzzy-matcher** - fuzzy string matching
- **serde / serde_json** - persistence
- **open** - open a URL in the browser
- **jni / android_logger / once_cell** - Android bridge

## Comparison with KISS Launcher (Android)

| Feature | KISS Android | This implementation |
|---------|--------------|---------------------|
| Search bar position | Bottom | Any position (top / bottom / middle / hidden) ✓ |
| Fuzzy search | Yes | Yes, with precision slider ✓ |
| Favorites bar | Yes | Yes, with capacity and shortcuts ✓ |
| Tags | Yes | Yes, with tag menu ✓ |
| Calculator | Yes | Yes ✓ |
| Settings search | Yes | Yes, from the query bar ✓ |
| Web / exec / timer providers | Yes | Yes ✓ |
| Usage tracking | Yes | Yes, with 3 ranking modes ✓ |
| Excluded apps | Yes | Yes ✓ |
| Import / export | Yes | Yes ✓ |
| Gestures | Yes | Yes, mapped to keyboard ✓ |
| Contact search | Yes | No (desktop focused) |

## Troubleshooting

### An application does not launch

- Check that the `Exec` line of the `.desktop` file works in a terminal
- `TryExec` entries whose binary is missing are hidden on purpose
- The status line under the search bar reports the error

### Applications are missing

- Press `F5` (or use the reload button of the info bar)
- Entries with `NoDisplay=true` or `Hidden=true` are skipped
- Excluded applications are searchable through the *Excluded apps* provider and
  can be restored from the settings

### Fonts

The launcher loads DejaVu Sans / Liberation / Noto on Linux and Roboto on
Android, and falls back to the egui default font when none is found.

## License

MIT License - feel free to use and modify as needed.

## Credits

- Inspired by [KISS Launcher](https://github.com/Neamar/KISS) for Android
- Built with [egui](https://github.com/emilk/egui) - an immediate mode GUI library
- Uses [fuzzy-matcher](https://github.com/lotabout/fuzzy-matcher) for search