# KISS Launcher - Quick Start Guide

## First Run

1. **Build the project:**
   ```bash
   cd kiss-launcher
   cargo build --release
   ```

2. **Run the launcher:**
   ```bash
   ./run.sh
   # or directly:
   ./target/release/kiss_launcher
   ```

## Basic Usage

### Searching for Apps
- Just start typing! The search bar is always focused
- Try: `fire` → Firefox, `term` → Terminal, `calc` → Calculator

### Launching Apps
- Press **Enter** to launch the highlighted app
- Or click on any app in the list

### Using the Calculator
Type math expressions:
- `2+2` → Shows `= 4`
- `100/7` → Shows `= 14.285714`
- `2^10` → Shows `= 1024`
- `(5+3)*2` → Shows `= 16`

### Managing Favorites
- Click the star (☆) next to any app to favorite it
- Favorites appear at the top when search is empty
- Click again (★) to remove from favorites

### Adding Tags
- Click the tag icon (🏷️) next to an app
- Type tags separated by commas: `web, internet, browser`
- Click "Save"
- Now you can search by tag!

## Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `↑` / `↓` | Navigate through results |
| `Enter` | Launch selected app |
| `Esc` | Clear search field |
| `Tab` | Cycle to next result |

## File Locations

- **History & Favorites:** `~/.config/kiss-launcher/history.json`
- **App Discovery:** Scans `/usr/share/applications` and `~/.local/share/applications`

## Tips & Tricks

1. **Launch Count Tracking**: The launcher automatically tracks which apps you use most and shows them first

2. **Fuzzy Matching**: Don't need to type exactly - `ffox` finds Firefox, `gimp` finds GIMP

3. **Multiple Search Terms**: Tags, categories, and app descriptions are all searchable

4. **Refresh Apps**: Click the 🔄 button in the top right to rescan for newly installed apps

5. **Minimal Design**: The search bar is at the bottom (like KISS Android) for easy thumb access on touchscreens

## Troubleshooting

**App won't launch?**
- Check if the app is in your PATH
- Look at terminal output for errors
- Try running the app directly from terminal first

**Apps not showing up?**
- Click 🔄 to refresh
- Check if the app has a .desktop file
- Some apps are hidden (NoDisplay=true)

**Font looks wrong?**
- The app tries to load DejaVu Sans
- Falls back to egui's default if not found
- You can customize in `src/ui.rs`

## Installation (Optional)

To install system-wide:
```bash
./install.sh
```

This will:
- Copy binary to `~/.local/bin/kiss_launcher`
- Add desktop entry to `~/.local/share/applications/`
- Make it available in your app menu

## Customization

### Change Colors
Edit `src/ui.rs` and modify the `Colors` struct:
```rust
struct Colors;
impl Colors {
    const BG: Color32 = Color32::from_rgb(30, 30, 46);
    const ACCENT: Color32 = Color32::from_rgb(137, 180, 250);
    // ... change these values
}
```

### Change Window Size
Edit `src/main.rs`:
```rust
.with_inner_size([420.0, 700.0])  // Change these values
```

Then rebuild: `cargo build --release`

## Examples

### Search Examples
```
fire    → Firefox, Firejail
git     → Git, GitKraken
term    → Terminal, Terminator
libre   → LibreOffice apps
```

### Calculator Examples
```
10*5      → = 50
144/12    → = 12
2^8       → = 256
(10+5)*2  → = 30
```

### Tag Examples
Tag Firefox with: `web, internet, browser, www`
Then search: `web` → Firefox appears!

## Performance

- **Startup time**: ~100-200ms (release build)
- **Memory usage**: ~30-50MB
- **Binary size**: ~7MB (release, stripped)
- **App scanning**: Instant (cached in memory)

## Next Steps

1. Add it to your window manager's keybindings (e.g., Super+Space)
2. Set it as your default launcher (if your DE supports it)
3. Customize colors to match your theme
4. Add custom tags to organize your workflow
5. Star your most-used apps for quick access

---

**Enjoy your minimalist launcher! ⚡**

For more details, see [README.md](README.md)
