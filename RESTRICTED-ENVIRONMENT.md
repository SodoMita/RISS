# Working in this restricted environment

This document records what it takes to build, verify and *visually* test RISS
inside a network-restricted sandbox (Arena agent mode), and how to test GUI
changes on machines that have no display. Everything here was verified by
actually doing it; commands are copy-pasteable.

## 1. The environment

Outbound HTTPS works only to an allowlist:

| Allowed | Blocked (examples) |
| --- | --- |
| `github.com`, `codeload.github.com`, `api.github.com` | `crates.io`, `static.crates.io`, `index.crates.io` |
| `registry.npmjs.org` | `static.rust-lang.org`, `sh.rustup.rs` |
| `pypi.org`, `files.pythonhosted.org` | `deb.debian.org`, any apt mirror |
| | `gitlab.freedesktop.org`, `docs.rs`, raw/release-asset CDNs |

Consequences to design around:

* **`cargo build` cannot fetch dependencies.** Rust sources must be vendored
  from GitHub (§3).
* **`rustup`/apt cannot install anything.** The toolchain comes from npm (§2).
* **No display stack is preinstalled**: no Xvfb, no Wayland compositor, no
  libEGL/libGL/libX11/libwayland, no `/dev/dri`. See §5 for what that means
  for GUI testing.
* GitHub *release assets* redirect to `release-assets.githubusercontent.com`,
  which is blocked. `codeload.github.com` (source tarballs) works fine.

### What is preinstalled (in the Arena sandbox)

`gcc`, `g++`, `make`, `git`, `gh` (authenticated), `python3`, `pip3`, `node`,
`npm`, `curl`, `unzip`, `xz`, ImageMagick (`import`, `convert`, `identify`),
and basic shell tools. No `cmake`, `meson`, `ninja`, `pkg-config` — see §2.3.

## 2. Installing the Rust toolchain

### 2.1 Download from npm (`@rustbin`)

[`@rustbin`](https://www.npmjs.com/search?q=%40rustbin) publishes the official
Rust dist components as npm tarballs. The registry is allowed, so this is the
reliable bootstrap:

```sh
mkdir -p tools && cd tools
for p in rustc cargo rust-std clippy rustfmt; do
    url=$(curl -s "https://registry.npmjs.org/@rustbin%2f${p}-1.88.0-x86_64-unknown-linux-gnu" \
        | python3 -c "import json,sys; d=json.load(sys.stdin); v=d['dist-tags']['latest']; print(d['versions'][v]['dist']['tarball'])")
    curl -sL -o "${p}.tgz" "$url"
done
```

Assemble the components into one prefix (same layout as a Rust dist tarball):

```sh
for p in rustc cargo rust-std clippy rustfmt; do
    mkdir -p x-$p && tar xzf "$p.tgz" -C x-$p
done
mkdir -p rust
cp -a x-rustc/package/rustc/.                          rust/
cp -a x-cargo/package/cargo/.                          rust/
cp -a x-rust-std/package/rust-std-x86_64-unknown-linux-gnu/. rust/
cp -a x-clippy/package/clippy-preview/.                rust/
cp -a x-rustfmt/package/rustfmt-preview/.              rust/
export PATH="$PWD/rust/bin:$PATH"
rustc --version   # rustc 1.88.0 (6b00bc388 2025-06-23)
cargo --version   # cargo 1.88.0 (873a06493 2025-05-10)
```

Note: `cargo`, `clippy` and `rustfmt` binaries are separate npm packages —
without the `rust-std` component `rustc` cannot link anything. The
`PATH` export must be repeated in every new shell.

`cargo fmt -- --check` needs no dependencies and works fully offline — it is
the one CI gate that can always be reproduced locally.

### 2.2 What still cannot be installed

* **crates.io dependencies** (egui, eframe, serde, …) — see §3.
* **distro packages** (apt is unreachable) — no X11/Wayland/GL libraries;
  see §5.

### 2.3 Build tools from PyPI

`meson` and `ninja` install from pypi (needed only if you build C libraries
from source):

```sh
pip3 install --user --break-system-packages meson ninja
export PATH="$HOME/.local/bin:$PATH"
```

(`--break-system-packages` is required because the sandbox Python is
externally managed. `cmake` and `pkg-config` are *not* on PyPI in usable form;
Meson-based projects do not need them.)

## 3. Offline cargo builds: the vendored-sources harness

`cargo` can build offline if every registry dependency is replaced by a local
path. Source tarballs of almost every crate are on GitHub (`codeload`), at
tags matching the version (dtolnay tags `1.0.107`, serde-rs tags `v1.0.229` —
probe both).

### 3.1 Recipe

1. Read exact versions from `Cargo.lock`.
2. Download each crate's repository tarball:

   ```sh
   curl -sL -o serde.tgz "https://codeload.github.com/serde-rs/serde/tar.gz/v1.0.229"
   tar xzf serde.tgz
   ```

3. In a scratch crate outside the repo (e.g. `/home/user/verify`), point
   every registry name at its vendored copy:

   ```toml
   [patch.crates-io]
   serde = { path = "vendor/serde-1.0.229/serde" }
   serde_core = { path = "vendor/serde-1.0.229/serde_core" }
   serde_derive = { path = "vendor/serde-1.0.229/serde_derive" }
   serde_json = { path = "vendor/json-1.0.151" }
   # …one line per crate in the dependency closure…
   ```

   Monorepos (serde) need one path per member crate; `Cargo.lock` may list
   several major versions of one crate (e.g. `syn` 2 *and* 3) — the `[patch]`
   table can only carry one, so pick the version your build actually needs.

4. Compile the *real* RISS sources with `#[path]` includes and run their
   tests:

   ```rust
   #[path = "/home/user/RISS/src/history.rs"]
   mod history;
   // …app_entry, providers, storage, settings, search…
   ```

   This catches type errors, borrow errors and lint issues in the core
   modules, and runs all unit tests, without ever touching crates.io.

Crates whose sources are not reachable can be stubbed with 20-line local
crates (`image`, `open` in the harness). The UI layer needs `eframe`/`egui`
whose full windowing dependency tree (winit, glutin, …) is too large to
vendor this way — UI code is verified by CI (§4).

## 4. CI as the compiler

GitHub Actions runners have full network and the `Tests` workflow runs
`cargo fmt`, `cargo clippy -- -D warnings`, `cargo test` and a
`cargo check --target aarch64-linux-android`. When a job fails but the logs
cannot be downloaded from the sandbox (the log CDN is not on the allowlist),
make the workflow re-emit diagnostics as **check annotations** — those are
readable through `api.github.com`:

```yaml
# .github/workflows/diag.yml (add temporarily)
name: Diagnostics
on: pull_request
jobs:
  diagnose:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
        with: { shared-key: "test-build" }
      - run: python3 ci/emit_annotations.py cargo check --all-targets --message-format=json
```

with `ci/emit_annotations.py`:

```python
#!/usr/bin/env python3
"""Re-emit rustc JSON messages as GitHub annotations."""
import json, subprocess, sys
proc = subprocess.run(sys.argv[1:], stdout=subprocess.PIPE,
                      stderr=subprocess.STDOUT, text=True)
count = 0
for line in proc.stdout.splitlines():
    try:
        msg = json.loads(line)
    except json.JSONDecodeError:
        continue
    if msg.get("reason") != "compiler-message":
        continue
    m = msg["message"]
    if m.get("level") not in ("error", "warning"):
        continue
    spans = m.get("spans") or []
    primary = next((s for s in spans if s.get("is_primary")),
                   spans[0] if spans else None)
    loc = f" file={primary['file_name']},line={primary['line_start']}" if primary else ""
    parts = [m.get("message", "")] + [c.get("message", "") for c in (m.get("children") or [])]
    text = " | ".join(p.replace("\n", " ").strip() for p in parts if p and p.strip())
    print(f"::{m['level']}{loc}::{text[:600]}")
    count += 1
    if count >= 40:
        break
sys.exit(0)
```

Read them back with:

```sh
gh api "repos/<owner>/<repo>/check-runs/<job-id>/annotations" \
    --jq '.[] | .annotation_level + " " + (.path // "?") + " :: " + .message'
```

(The job id comes from `gh api repos/…/actions/runs/<run>/jobs`.)

## 5. GUI testing

### 5.1 Why the launcher cannot run inside the sandbox

RISS renders through `eframe`/`glow`, i.e. it needs an **OpenGL** context at
runtime. A GUI run therefore needs *both* a display server *and* an EGL/GL
stack. Verified state of the sandbox:

* no display binaries or libraries at all (no Xvfb, no compositor, no
  libEGL/libGL/libX11/libwayland/libxkbcommon), no `/dev/dri`;
* the Wayland stack itself **can** be built from source — mirrors exist:
  `freedesktop-unofficial-mirror/wayland__wayland`,
  `IcebergThings/wayland-protocols`, `xkbcommon/libxkbcommon`,
  `libffi/libffi`, `libexpat/libexpat` (fetch via `codeload`, build with the
  pip-installed meson/ninja);
* **mesa has no reachable source mirror** (`mesa3d/mesa` does not exist on
  GitHub, `gitlab.freedesktop.org` is blocked) — so no OpenGL
  implementation can be built here. That is the hard wall.

A pure-`wl_shm` compositor can be compiled and run in-sandbox, but it cannot
display RISS (an EGL client). Running a compositor here only makes sense as a
protocol playground.

### 5.2 Small Wayland compositors — what exists

| Compositor | Size | Needs | Good for |
| --- | --- | --- | --- |
| **cage** | tiny (kiosk, runs one app fullscreen) | wlroots | **best fit** for smoke-testing one app |
| **tinywl** (wlroots example) | ~1.2 kLOC C | wlroots | understanding/patching a compositor |
| **harmony** (`osakpwn/harmony`) | one `harmony.c` | wlroots 0.20, wayland-server, xkbcommon | smallest real-world example |
| **Zen** (`Darianopolis/Zen`) | small | wlroots | a readable modern compositor |
| **weston** (`--backend=headless`) | reference | pixman or GL | protocol-complete headless |
| sway / labwc / river | full DE-ish | wlroots | full desktop behaviour |

All of the "tiny" ones are built on **wlroots**, which itself needs
libwayland, wayland-protocols, libdrm, pixman, xkbcommon *and* mesa — in
other words the same GL wall as §5.1. Source links for the from-source
curious:

* `cage`: apt package `cage` (Ubuntu ≥ 22.04), sources on Codeberg
* wlroots: `swaywm/wlroots` (mirror; canonical is gitlab.freedesktop.org)
* tinywl ships inside the wlroots tree (`tinywl/tinywl.c`)
* `harmony`: `https://github.com/osakpwn/harmony`
* weston mirrors: `IcebergThings/weston`, `freedesktop-unofficial-mirror/wayland__weston`

### 5.3 The working recipe: headless GUI smoke on CI

GitHub's `ubuntu-latest` runners have apt, mesa (llvmpipe software GL) and
Xvfb. Two headless display options, both scripted in
`scripts/gui-smoke-test.sh`:

**X11 (Xvfb)** — install `xvfb imagemagick`, then:

```sh
xvfb-run -a -s "-screen 0 1280x720x24" ./target/debug/riss_launcher &
sleep 8 && import -window root screenshot.png   # ImageMagick grabs the screen
```

**Wayland (cage + grim)** — install `cage grim`, then:

```sh
XDG_RUNTIME_DIR=$(mktemp -d) chmod 700 "$XDG_RUNTIME_DIR"
WLR_BACKENDS=headless WLR_LIBINPUT_NO_DEVICES=1 WLR_RENDERER_ALLOW_SOFTWARE=1 \
    cage ./target/debug/riss_launcher &
sleep 8 && grim screenshot.png                  # wlr-screencopy
```

Notes:

* `WLR_BACKENDS=headless` gives wlroots a virtual output (no GPU needed);
* `WLR_RENDERER_ALLOW_SOFTWARE=1` lets the GL renderer fall back to Mesa's
  llvmpipe — the client (RISS) creates EGL contexts the compositor imports;
* `WLR_LIBINPUT_NO_DEVICES=1` lets the compositor start without any input
  devices;
* the same commands work on any Linux box with `apt install cage grim xvfb`.

The `gui-smoke` job in `.github/workflows/test.yml` runs both modes, saves the
screenshots as the **`smoke-screenshots` workflow artifact** (downloadable
from the Actions page), and prints image statistics as workflow annotations
(`mean`, `stddev` per screenshot) so remote callers can confirm that pixels
were actually rendered:

```sh
convert shot.png -colorspace RGB -format "%[fx:mean] %[fx:standard_deviation]" info:
```

### 5.4 Future: offscreen rendering without a compositor

The remaining in-sandbox option is to render the egui frame *offscreen*:
`egui::Context::run` + `epaint::tessellate` produce triangle meshes that can
be rasterized in ~200 lines of software code and written out as PNG — no
window system, no GL. It needs `egui`/`epaint` vendored (§3; a much smaller
tree than `eframe`) and a thin `RissApp::ui(&self, ctx)` seam so the frame
loop does not need `eframe::Frame`. This is the recommended direction if
pixel-level in-sandbox testing becomes necessary.

## 6. Quick reference

```sh
# toolchain
curl … @rustbin …          # §2.1, then: export PATH=…/tools/rust/bin:$PATH
cargo fmt -- --check       # works offline

# core-module verification without crates.io
cd /home/user/verify && cargo test --offline   # §3

# GUI smoke on any Linux with a display or headless apt access
cargo build && bash scripts/gui-smoke-test.sh xvfb     # §5.3
cargo build && bash scripts/gui-smoke-test.sh wayland  # §5.3

# remote debugging
gh api "repos/<o>/<r>/check-runs/<id>/annotations" --jq '.[]|.message'   # §4
```

## Appendix: merging the rest of PR #3 (icons)

PR #3 also carries an icon layer that is *not* part of the provider port and
merges cleanly on top of it later:

* `src/ui/widgets.rs` — `Icon` enum (34 vector glyphs drawn with the egui
  painter: Search, Star, Tag, Clock, Terminal, Globe, …), `draw_icon`,
  `app_badge` (letter badge), `icon_button`, and small UI helpers;
* `src/theme.rs` — `BADGE_COLORS` + `badge_color(name)`, an FNV-1a hash that
  gives every app a deterministic badge colour (8 muted Catppuccin-ish
  tones);
* call sites in `src/ui/results.rs` replace the inline letter-badge fallback
  and the text buttons (★, ⋮) with the vector icons.

Nothing in that layer touches the real-icon pipeline
(`app_entry::load_icon_rgba`), which the current branch keeps exactly as main
has it; provider rows (`AppEntry::is_virtual`) are the natural place to plug
the vector icons in. So: yes — the icon branch can be ported incrementally on
top of this work without replacing anything.
