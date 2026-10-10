#!/usr/bin/env bash
# Headless GUI assertions for RISS: run the launcher under sway on a virtual
# output and fail the build on measured pixel evidence, not on "the process
# lived". Each check exists because the bug it catches got through a green
# `cargo test`.
#
#   scripts/gui-assertions.sh [path-to-binary]
#
# Exit codes: 0 all assertions passed (or the compositor could not start and a
# loud ::warning:: was emitted), 1 an assertion failed or the launcher crashed.
# No wall-clock thresholds anywhere: they are the first flake a CI job grows.
set -u

BIN="${1:-$(dirname "$0")/../target/debug/riss_launcher}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT="${ROOT}/gui-artifacts"
mkdir -p "$OUT"

note() { echo "::notice::gui-assert: $*"; }
warn() { echo "::warning::gui-assert: $*"; }
fail() {
    echo "::error::gui-assert: $*"
    [ -n "${CASE_LOG:-}" ] && [ -s "$CASE_LOG" ] && {
        echo "::error::launcher log tail:"
        tail -n 15 "$CASE_LOG" | sed 's/^/%0A  /'
    }
    exit 1
}

for tool in sway grim identify convert; do
    command -v "$tool" >/dev/null || { warn "$tool missing; GUI assertions skipped (apt-get install -y sway grim imagemagick)"; exit 0; }
done
[ -x "$BIN" ] || fail "binary not found at $BIN; run 'cargo build' first"

# One sway session per case: a compositor that survives a crash is worth less
# than a clean slate, and the sessions are cheap.
start_session() { # $1 = bg color, $2 = case dir, $3 = optional "WxH" mode
    local bg="$1" case_dir="$2" mode="${3:-}"
    # Start from a clean environment: sway honours an inherited SWAYSOCK or
    # WAYLAND_DISPLAY from a previous case and puts its sockets somewhere else
    # entirely, which then looks like a compositor that will not start.
    unset SWAYSOCK WAYLAND_DISPLAY
    export XDG_RUNTIME_DIR="$case_dir/runtime"
    mkdir -p "$XDG_RUNTIME_DIR"; chmod 700 "$XDG_RUNTIME_DIR"
    export WLR_BACKENDS=headless WLR_RENDERER_ALLOW_SOFTWARE=1 LIBGL_ALWAYS_SOFTWARE=1 \
           GALLIUM_DRIVER=llvmpipe WLR_LIBINPUT_NO_DEVICES=1
    # xwayland disable: the launcher is a native Wayland client, and a runner
    # without Xwayland installed makes sway's own X server a fatal startup error
    # rather than an unused feature.
    write_config() {
        {
            printf 'output * bg %s solid_color\n' "$bg"
            [ -n "$1" ] && printf 'output * res %s\n' "$1"
            printf 'xwayland disable\ndefault_border none\ninput * xkb_layout us\n'
        } > "$XDG_RUNTIME_DIR/config"
    }

    # A headless backend may refuse a mode request (wlroots' default set varies
    # by version, and sway 1.7 rejects "@60" on some outputs). The assertions
    # read the geometry the compositor actually produced, so a refused mode is
    # a weaker test rather than a broken one -- say so and carry on.
    local attempt
    for attempt in with-mode without-mode; do
        case "$attempt" in
            with-mode) [ -n "$mode" ] || continue; write_config "$mode" ;;
            without-mode) write_config "" ;;
        esac
        SWAYSOCK=""; WAYLAND_DISPLAY=""
        rm -f "$XDG_RUNTIME_DIR"/wayland-* "$XDG_RUNTIME_DIR"/sway-ipc.* 2>/dev/null
        sway --config "$XDG_RUNTIME_DIR/config" > "$case_dir/compositor.$attempt.log" 2>&1 &
        COMPOSITOR=$!
        # Wait for BOTH sockets: the client display is created a moment after
        # the IPC one, and launching the app against the stale name from a
        # previous attempt is a race that looks like a crash.
        for _ in $(seq 1 60); do
            SWAYSOCK="$(find "$XDG_RUNTIME_DIR" -maxdepth 1 -name 'sway-ipc.*' 2>/dev/null | head -1)"
            WAYLAND_DISPLAY="$(ls "$XDG_RUNTIME_DIR" 2>/dev/null | grep -E '^wayland-[0-9]+$' | head -1)"
            [ -n "$SWAYSOCK" ] && [ -n "$WAYLAND_DISPLAY" ] && break
            kill -0 "$COMPOSITOR" 2>/dev/null || break
            sleep 0.25
        done
        if [ -z "$SWAYSOCK" ] || [ -z "$WAYLAND_DISPLAY" ]; then
            kill "$COMPOSITOR" 2>/dev/null; wait "$COMPOSITOR" 2>/dev/null; continue
        fi
        export SWAYSOCK
        export WAYLAND_DISPLAY
        [ "$attempt" = without-mode ] && [ -n "$mode" ] && \
            note "requested output mode ${mode} was refused; continuing at the compositor's default"
        return 0
    done
    return 1
}

run_case() { # $1 = tag, $2 = bg, $3 = settings json, $4 = optional output mode
    local tag="$1" bg="$2" settings="$3" mode="${4:-}"
    local dir="$OUT/$tag"; mkdir -p "$dir"; export RISS_DATA_DIR="$dir/data"
    mkdir -p "$RISS_DATA_DIR"
    printf '%s' "$settings" > "$RISS_DATA_DIR/settings.json"
    CASE_LOG="$dir/launcher.log"
    if ! start_session "$bg" "$dir" "$mode"; then
        warn "compositor would not start (see $dir/compositor.*.log); GUI assertions did NOT run"
        exit 0
    fi
    timeout 60 "$BIN" > "$CASE_LOG" 2>&1 &
    local app=$!
    sleep 9
    kill -0 "$app" 2>/dev/null || { kill $COMPOSITOR 2>/dev/null; fail "[$tag] launcher exited before it could be measured"; }
    # The window must actually be mapped: "it runs" is not "it draws".
    if command -v swaymsg >/dev/null; then
        swaymsg -t get_tree 2>/dev/null | grep -q 'RISS Launcher' \
            || { kill "$app" $COMPOSITOR 2>/dev/null; fail "[$tag] no RISS window in the sway tree"; }
    fi
    timeout 20 grim "$dir/screen.png" 2>>"$CASE_LOG" || { kill "$app" $COMPOSITOR 2>/dev/null; fail "[$tag] grim produced nothing"; }
    SHOT="$dir/screen.png"; window_rect
    if [ -n "$WIN_GEOM" ]; then
        GEOM="${WIN_GEOM#-crop }"; GEOM="${GEOM% +repage}"
        WIN_W="${GEOM%%x*}"; WIN_HX="${GEOM#*x}"; WIN_H="${WIN_HX%%+*}"
        OFFS="${GEOM##*+}"; WIN_X="${OFFS%%+*}"; WIN_Y="${OFFS##*+}"
        note "[$tag] measuring the window rect ($GEOM)"
    else
        read -r WIN_W WIN_H <<<"$(identify -format '%w %h' "$dir/screen.png")"
        WIN_X=0; WIN_Y=0; GEOM="${WIN_W}x${WIN_H}+0+0"
        note "[$tag] compositor gave no window rect; measuring the full frame"
    fi
    kill "$app" $COMPOSITOR 2>/dev/null; wait 2>/dev/null
    return 0
}

# Where the launcher window actually is. sway tiles a lone window across the
# output, but a desktop compositor may float it, and then measuring the whole
# frame would credit the desktop behind it. So: ask the compositor for the
# window rect when it can say, and fall back to the frame.
WIN_GEOM=""
window_rect() {
    WIN_GEOM=""
    command -v swaymsg >/dev/null || return 0
    WIN_X=0; WIN_Y=0; WIN_W=0; WIN_H=0
    local rect
    rect="$(swaymsg -t get_tree 2>/dev/null | python3 -c '
import json,sys
def walk(n):
    yield n
    for k in ("nodes","floating_nodes"):
        for c in n.get(k) or []:
            yield from walk(c)
try:
    tree=json.load(sys.stdin)
except Exception:
    raise SystemExit("")
for n in walk(tree):
    if n.get("type")=="con" and "RISS" in (n.get("name") or ""):
        wr, pr = n.get("window_rect") or {}, n.get("rect") or {}
        if wr.get("width",0) > 40:
            print("%dx%d+%d+%d" % (wr["width"], wr["height"],
                                   pr.get("x",0)+wr.get("x",0), pr.get("y",0)+wr.get("y",0)))
            break
' 2>/dev/null)"
    [ -n "$rect" ] && WIN_GEOM="-crop $rect +repage"
}

# red fraction of the launcher window: any pixel it does not paint comes back
# red, so this measures real window alpha rather than an option in a builder.
red_fraction() {
    convert "$SHOT" -crop "$GEOM" +repage -channel R -separate +channel -threshold 70% -format '%[fx:mean*100]' info:
}
# distinct colours inside a box: a hidden glyph leaves a flat fill (1-2
# colours), a drawn one has strokes and antialiasing.
# Distinct colours inside an 84px box anchored to a window corner: a hidden
# glyph leaves a flat fill (1-2 colours), a drawn one has strokes and
# antialiasing. Coordinates are absolute so no gravity semantics are involved.
BOX=56
box_colors() { # $1 = left|right  -- a small box centred on the bar's icon slot
    local x
    if [ "$1" = left ]; then x=$((WIN_X + 16)); else x=$((WIN_X + WIN_W - 16 - BOX)); fi
    local y=$((WIN_Y + WIN_H - 66))
    convert "$SHOT" -crop ${BOX}x${BOX}+${x}+${y} +repage -format '%k' info:
}

note "case 1: transparent theme must let the desktop through"
run_case transparent "#ff0000" '{"bools":{},"values":{"theme":"transparent","theme-wallpaper":"show"}}'
red=$(red_fraction)
awk -v r="$red" 'BEGIN{exit !(r>=40)}' || fail "[transparent] only ${red}% of the frame is red: the window is not blending (issue #32/#25). Expect ~90% with the transparent theme."
note "[transparent] red through the window: ${red}% (>= 40 required)"

note "case 2: solid themes must stay opaque"
run_case dark "#ff0000" '{"bools":{},"values":{"theme":"dark","theme-wallpaper":"show"}}'
red=$(red_fraction)
awk -v r="$red" 'BEGIN{exit !(r<=3)}' || fail "[dark] ${red}% red: an opaque theme is leaking the desktop through"
note "[dark] red: ${red}% (<= 3 required)"

note "case 3: theme-wallpaper=hide must paint a solid panel"
run_case hide "#ff0000" '{"bools":{},"values":{"theme":"transparent","theme-wallpaper":"hide"}}'
red=$(red_fraction)
awk -v r="$red" 'BEGIN{exit !(r<=3)}' || fail "[hide] ${red}% red: KISS 'hide' is being ignored"
note "[hide] red: ${red}% (<= 3 required)"

# Differential on purpose. An absolute "how many colours are in that corner"
# threshold depends on where the crop lands, the bar radius and the theme, and a
# flaky geometry test gets deleted rather than fixed. Comparing two frames from
# the same build at the same offsets answers the actual question: did the ring
# disappear and did the cog stay.
note "case 4: pref-hide-circle must hide the ring and keep the cog drawn (#55)"
run_case circles-off "#0d1117" '{"bools":{"pref-hide-circle":false},"values":{"theme":"dark"}}'
ring_shown=$(box_colors left); cog_shown=$(box_colors right)
run_case circles-on "#0d1117" '{"bools":{"pref-hide-circle":true},"values":{"theme":"dark"}}'
ring_hidden=$(box_colors left); cog_hidden=$(box_colors right)
awk -v a="$ring_hidden" -v b="$ring_shown" 'BEGIN{exit !(a < b)}' \
    || fail "[hide-circle] left slot went from $ring_shown to $ring_hidden colours: hiding the circle changed nothing"
awk -v a="$cog_hidden" -v b="$cog_shown" 'BEGIN{exit !(a >= b * 0.8)}' \
    || fail "[hide-circle] the cog slot fell from $cog_shown to $cog_hidden colours: hiding the circle also hid the settings button (#55)"
note "[hide-circle] ring $ring_shown -> $ring_hidden colours, cog steady at $cog_shown -> $cog_hidden"

note "case 5: the bar must survive a 360px-tall output (#50/#51 class)"
run_case tiny "#0d1117" '{"bools":{},"values":{"theme":"dark"}}' 800x360
bar=$(convert "$SHOT" -crop ${WIN_W}x64+${WIN_X}+$((WIN_Y + WIN_H - 64)) +repage -format '%k' info:)
awk -v c="$bar" 'BEGIN{exit !(c>=8)}' || fail "[tiny output] bottom 64px has only $bar colours: the search bar is clipped off screen"
note "[tiny] bottom strip colours=$bar"

echo "gui-assert: all cases passed"
