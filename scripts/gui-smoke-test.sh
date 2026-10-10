#!/usr/bin/env bash
# Launch RISS under a headless display, screenshot it, and report image
# statistics as GitHub Actions annotations.
#
# Usage: scripts/gui-smoke-test.sh [xvfb|wayland]
#
#   xvfb    X11 session on a virtual framebuffer (needs xvfb + imagemagick)
#   wayland wlroots headless session in cage (needs cage + grim)
#
# Screenshots land in smoke-artifacts/; upload them as a workflow artifact to
# inspect them. The exit code is non-zero when the launcher crashes before
# the screenshot or when no screenshot can be taken.

set -u

MODE="${1:-xvfb}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BIN="${ROOT}/target/debug/riss_launcher"
OUT="${ROOT}/smoke-artifacts"
LOG="${OUT}/smoke-${MODE}.log"
SHOT="${OUT}/smoke-${MODE}.png"
mkdir -p "$OUT"

note() { echo "::notice::gui-smoke (${MODE}): $*"; }
warn() { echo "::warning::gui-smoke (${MODE}): $*"; }
fail() {
    echo "::error::gui-smoke (${MODE}): $*"
    if [ -s "$LOG" ]; then
        grep -Ei "error|fail|panic|cannot|unable" "$LOG" | grep -vEi "keysym|xkbcomp" | tail -n 15 | while IFS= read -r line; do
            echo "::error::log: ${line}"
        done
        echo "::error::log: ---- raw tail ----"
        grep -vEi "keysym|xkbcomp" "$LOG" | tail -n 12 | while IFS= read -r line; do
            echo "::error::log: ${line}"
        done
    fi
    exit 1
}

# Start the launcher ($1) logging to $2; after 8s take the screenshot with
# the command in $3 and stop the launcher. If the launcher exits on its own
# first, propagate its exit code (that is a crash or a startup failure).
run_session() {
    bash -c '
        eval "$1" >"$2" 2>&1 &
        APP=$!
        sleep 8
        if kill -0 "$APP" 2>/dev/null; then
            eval "$3" >>"$2" 2>&1
            kill "$APP" 2>/dev/null
            wait "$APP" 2>/dev/null
            exit 0
        fi
        wait "$APP"
        exit $?
    ' _ "$@"
}

[ -x "$BIN" ] || [ "$MODE" = "--run-child" ] || fail "binary not found at ${BIN}; run 'cargo build' first"

case "$MODE" in
    xvfb)
        command -v xvfb-run >/dev/null || fail "xvfb-run missing (apt install xvfb)"
        command -v import >/dev/null || fail "imagemagick import missing"
        xvfb-run -a -s "-screen 0 1280x720x24" \
            bash "$0" --run-child "$BIN" "$LOG" "$SHOT" "import -display \$DISPLAY -window root" \
            2>>"$LOG" || fail "Xvfb session failed (see log)"
        ;;
    wayland)
        command -v cage >/dev/null || fail "cage missing (apt install cage)"
        command -v grim >/dev/null || fail "grim missing (apt install grim)"
        export XDG_RUNTIME_DIR="${XDG_RUNTIME_DIR:-$(mktemp -d)}"
        chmod 700 "$XDG_RUNTIME_DIR"
        # Native path: the launcher is a Wayland/EGL client, so push Mesa to
        # its software renderer (llvmpipe) and let wlroots accept it. Success
        # is defined by the screenshot existing — cage's exit code alone is
        # not reliable.
        WLR_BACKENDS=headless \
            WLR_LIBINPUT_NO_DEVICES=1 \
            WLR_RENDERER_ALLOW_SOFTWARE=1 \
            LIBGL_ALWAYS_SOFTWARE=1 \
            GALLIUM_DRIVER=llvmpipe \
            cage -- bash "$0" --run-child "$BIN" "$LOG" "$SHOT" "grim" \
            2>>"$LOG"
        native=$?
        if [ -s "$SHOT" ]; then
            [ "$native" -ne 0 ] && warn "cage exited ${native} but a screenshot exists"
            note "session: native Wayland (cage + llvmpipe)"
        else
            # Fallback: run the launcher as an X11 client on cage's Xwayland
            # (pure software: pixman compositor + glamor sw fallback). Only
            # the launcher loses WAYLAND_DISPLAY — grim still needs it.
            warn "native Wayland produced no screenshot (cage exit ${native}) — retrying through Xwayland"
            mv "$LOG" "${LOG%.log}.native.log"
            WLR_BACKENDS=headless \
                WLR_LIBINPUT_NO_DEVICES=1 \
                WLR_RENDERER=pixman \
                cage -- bash "$0" --run-child \
                    "env -u WAYLAND_DISPLAY DISPLAY=${DISPLAY:-:0} $(printf '%q' "$BIN")" \
                    "$LOG" "$SHOT" "grim" \
                2>>"$LOG"
            x11=$?
            [ -s "$SHOT" ] || fail "no screenshot from either path (cage exits: native ${native}, xwayland ${x11})"
            [ "$x11" -ne 0 ] && warn "cage exited ${x11} but a screenshot exists"
            note "session: Xwayland inside cage (native wayland path unavailable)"
        fi
        ;;
    --run-child)
        # Internal: runs inside xvfb-run/cage. $2 bin (command), $3 log,
        # $4 shot, $5 screenshot command.
        echo "::notice::gui-smoke (session): child started, pwd=$(pwd), DISPLAY=${DISPLAY:-unset}, WAYLAND_DISPLAY=${WAYLAND_DISPLAY:-unset}"
        LOG="$3"
        SHOT="$4"
        run_session "$2" "$3" "$5 $(printf '%q' "$4")" \
            || fail "launcher exited abnormally (see log)"
        [ -s "$4" ] || fail "no screenshot was produced"
        exit 0
        ;;
    *)
        fail "unknown mode '${MODE}' (use xvfb or wayland)"
        ;;
esac

[ -s "$SHOT" ] || fail "no screenshot was produced"

if command -v convert >/dev/null 2>&1; then
    dims=$(identify -format "%wx%h" "$SHOT" 2>/dev/null)
    mean=$(convert "$SHOT" -colorspace RGB -format "%[fx:mean]" info: 2>/dev/null)
    sd=$(convert "$SHOT" -colorspace RGB -format "%[fx:standard_deviation]" info: 2>/dev/null)
    note "screenshot ${dims} mean=${mean:-?} stddev=${sd:-?}"
fi
note "ok — ${SHOT}"
