#!/usr/bin/env bash
# Pixel assertions for the live application search: type real queries into
# the RUNNING launcher under headless sway and measure what it painted.
#
#   scripts/gui-search-assertions.sh [path-to-binary]
#
# Companion to gui-assertions.sh, which proves the launcher starts and paints
# but injects no keystrokes. This one needs `wtype` (virtual-keyboard
# protocol) on top of sway/grim/imagemagick; without it the script exits 0
# with a loud ::warning:: so runner packaging drift never reddens an unrelated
# change -- but then it measured nothing, and the summary says so.
#
# What is asserted, and why it is measurable without OCR: a result row for an
# installed application carries the ★ and ⋮ buttons at its right edge; a
# provider row (web search) does not. So "the first row for `cafe` is an app,
# not the web-search fallback" is a colour count in the first row's right
# margin, compared against a query nothing matches.
#
# Known input quirk, learned by running it: wtype drops the first key of each
# invocation (the compositor has not applied the virtual keyboard's keymap
# yet), so every invocation starts with `-s 400`. The first draft of this
# script read "afe" back for "cafe" and nearly reported a launcher bug.
set -u

BIN="${1:-$(dirname "$0")/../target/debug/riss_launcher}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT="${ROOT}/gui-artifacts/search"
rm -rf "$OUT"; mkdir -p "$OUT"

note() { echo "::notice::gui-search: $*"; }
warn() { echo "::warning::gui-search: $*"; }
fail() {
    echo "::error::gui-search: $*"
    [ -s "$OUT/launcher.log" ] && { echo "::error::launcher log tail:"; tail -n 15 "$OUT/launcher.log" | sed 's/^/%0A  /'; }
    kill "${APP:-}" "${COMP:-}" 2>/dev/null
    exit 1
}

for tool in sway grim identify convert wtype; do
    command -v "$tool" >/dev/null || { warn "$tool missing; search pixel assertions skipped (apt-get install -y sway grim imagemagick wtype)"; exit 0; }
done
[ -x "$BIN" ] || fail "binary not found at $BIN; run 'cargo build' first"

# --- a controlled app list -------------------------------------------------
# Discovery reads $XDG_DATA_DIRS/applications and dedups on Exec, so every
# entry gets its own Exec line (six apps sharing /bin/true collapse to one).
APPS="$OUT/share/applications"; mkdir -p "$APPS"
mk() { printf '[Desktop Entry]\nType=Application\nName=%s\nExec=%s\nIcon=none\n' "$1" "$2" > "$APPS/$3.desktop"; }
mk "Café Noir" "/bin/true cafe" cafe
mk "Firefox" "/bin/true firefox" firefox
mk "Files" "/bin/true files" files
mk "Nautilus" "/bin/true nautilus" nautilus
mk "Visual-Studio" "/bin/true vs" vs
mk "VirtualBox" "/bin/true vbox" vbox
export XDG_DATA_DIRS="$OUT/share" XDG_DATA_HOME="$OUT/nohome" HOME="$OUT/home"; mkdir -p "$HOME"
export RISS_DATA_DIR="$OUT/data"; mkdir -p "$RISS_DATA_DIR"
# display-keyboard: focus the field on the first frame so typing lands in it.
printf '{"bools":{"display-keyboard":true},"values":{"theme":"dark"}}' > "$RISS_DATA_DIR/settings.json"

# --- compositor ------------------------------------------------------------
unset SWAYSOCK WAYLAND_DISPLAY
export XDG_RUNTIME_DIR="$OUT/runtime"; mkdir -p "$XDG_RUNTIME_DIR"; chmod 700 "$XDG_RUNTIME_DIR"
export WLR_BACKENDS=headless WLR_RENDERER_ALLOW_SOFTWARE=1 LIBGL_ALWAYS_SOFTWARE=1 \
       GALLIUM_DRIVER=llvmpipe WLR_LIBINPUT_NO_DEVICES=1
printf 'output * bg #ff0000 solid_color\nxwayland disable\ndefault_border none\ninput * xkb_layout us\n' > "$XDG_RUNTIME_DIR/config"
sway --config "$XDG_RUNTIME_DIR/config" > "$OUT/compositor.log" 2>&1 &
COMP=$!
for _ in $(seq 1 60); do
    SWAYSOCK="$(find "$XDG_RUNTIME_DIR" -maxdepth 1 -name 'sway-ipc.*' 2>/dev/null | head -1)"
    WAYLAND_DISPLAY="$(ls "$XDG_RUNTIME_DIR" 2>/dev/null | grep -E '^wayland-[0-9]+$' | head -1)"
    [ -n "$SWAYSOCK" ] && [ -n "$WAYLAND_DISPLAY" ] && break
    kill -0 "$COMP" 2>/dev/null || break
    sleep 0.25
done
if [ -z "${SWAYSOCK:-}" ] || [ -z "${WAYLAND_DISPLAY:-}" ]; then
    warn "compositor would not start (see $OUT/compositor.log); search pixel assertions did NOT run"
    exit 0
fi
export SWAYSOCK WAYLAND_DISPLAY

timeout 120 "$BIN" > "$OUT/launcher.log" 2>&1 &
APP=$!
sleep 9
kill -0 "$APP" 2>/dev/null || fail "launcher exited before it could be typed into"

# --- typing ----------------------------------------------------------------
type_query() { # $1 = text; the field is emptied first
    wtype -s 400 -k BackSpace -k BackSpace -k BackSpace -k BackSpace -k BackSpace -k BackSpace -k BackSpace -k BackSpace
    sleep 0.3
    wtype -s 400 -d 60 "$1"
    sleep 2
}
shoot() { timeout 20 grim "$OUT/$1.png" 2>>"$OUT/launcher.log" || fail "grim produced nothing for $1"; }

read -r W H <<<"$(sleep 0.5; grim - | identify -format '%w %h' -)"
# The first result row: top strip of the window. Its right margin holds ★ ⋮
# for an application row and nothing for a provider row.
ROW_H=$(( H / 12 )); MARGIN_W=$(( W / 8 ))
right_margin_colours() { # $1 = shot tag
    convert "$OUT/$1.png" -crop "${MARGIN_W}x${ROW_H}+$(( W - MARGIN_W ))+0" +repage -format '%k' info:
}
top_row_colours() {
    convert "$OUT/$1.png" -crop "$(( W - MARGIN_W ))x${ROW_H}+0+0" +repage -format '%k' info:
}

shoot empty
type_query "qzxjw";  shoot nomatch
type_query "cafe";   shoot cafe
type_query "vb";     shoot vb
kill -0 "$APP" 2>/dev/null || fail "launcher died while being typed into"

EMPTY_RIGHT=$(right_margin_colours empty)
NOMATCH_RIGHT=$(right_margin_colours nomatch)
CAFE_RIGHT=$(right_margin_colours cafe)
VB_RIGHT=$(right_margin_colours vb)
CAFE_TOP=$(top_row_colours cafe)
note "first-row right-margin colours -- empty: $EMPTY_RIGHT | no-match (provider row): $NOMATCH_RIGHT | cafe: $CAFE_RIGHT | vb: $VB_RIGHT (cafe first row overall: $CAFE_TOP)"

kill "$APP" "$COMP" 2>/dev/null; wait 2>/dev/null

# The thresholds are relative: an application row's ★ ⋮ margin must hold
# several times the colours of a provider row's bare margin. (An earlier
# absolute version asserted the empty state painted nothing up top -- it
# paints the "Swipe up for all apps" hint there, 234 colours' worth. Measure
# first, then write the number down.)
# 1. A query nothing matches falls through to the web-search row, which has
#    no action buttons: its margin must look like the empty state's.
[ "$NOMATCH_RIGHT" -le $(( EMPTY_RIGHT + 24 )) ] || fail "a no-match query's first row grew action buttons ($NOMATCH_RIGHT margin colours vs $EMPTY_RIGHT empty); only a provider row should be there"
# 2. "cafe" must find Café Noir: an APPLICATION row first, buttons and all.
[ "$CAFE_TOP" -gt 8 ] || fail "\"cafe\" painted no first row at all ($CAFE_TOP colours)"
[ "$CAFE_RIGHT" -ge $(( NOMATCH_RIGHT * 3 )) ] && [ "$CAFE_RIGHT" -ge 40 ] \
    || fail "\"cafe\" did not put an application first ($CAFE_RIGHT margin colours vs $NOMATCH_RIGHT for a provider row): accent folding is broken"
# 3. "vb" must find VirtualBox through the camel-case bonus.
[ "$VB_RIGHT" -ge $(( NOMATCH_RIGHT * 3 )) ] && [ "$VB_RIGHT" -ge 40 ] \
    || fail "\"vb\" did not put an application first ($VB_RIGHT margin colours vs $NOMATCH_RIGHT): camel-case matching is broken"

note "all search pixel cases passed (screens in $OUT)"
echo "gui-search: all cases passed"
