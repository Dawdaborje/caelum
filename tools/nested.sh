#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Run a binary inside a nested KWin window, without touching the real desktop.
# Usage: tools/nested.sh [--outputs N] <binary> [args...]
# If NESTED_PIDFILE is set, the binary's PID is written there once it starts.
set -euo pipefail

outputs=1
if [[ ${1:-} == --outputs ]]; then outputs=$2; shift 2; fi
[[ $# -ge 1 ]] || { echo "usage: $0 [--outputs N] <binary> [args...]" >&2; exit 2; }

bin=$1; shift
[[ $bin == */* ]] && bin=$(realpath "$bin")

wrapper=$(mktemp --suffix=.sh)
kwin_cfg=$(mktemp -d)
kwin_pid=
dbus_pid=
cleanup() {
    [[ -n $kwin_pid ]] && kill "$kwin_pid" 2>/dev/null && wait "$kwin_pid" 2>/dev/null
    [[ -n $dbus_pid ]] && kill "$dbus_pid" 2>/dev/null
    rm -rf "$wrapper" "$kwin_cfg"
}
trap cleanup EXIT
trap 'exit 130' INT TERM

# A private session bus, so the nested KWin can't take D-Bus names from the real desktop.
mapfile -t bus < <(dbus-daemon --session --fork --print-address=1 --print-pid=1)
export DBUS_SESSION_BUS_ADDRESS=${bus[0]}
dbus_pid=${bus[1]}

# KWin gets its own config dir, so it never writes to ~/.config or loads the real
# desktop's plugins. Caelestia's tracker effect is on by default and would connect
# to the running Caelestia, so turn it off explicitly.
printf '[Plugins]\nkwin_workspace_trackerEnabled=false\n' > "$kwin_cfg/kwinrc"

# --exit-with-session takes one path, so the command line goes in a wrapper.
# $$ survives exec, so it is the binary's PID.
{
    printf '#!/bin/sh\n'
    # The binary under test gets the caller's real config dir back.
    if [[ -n ${XDG_CONFIG_HOME:-} ]]; then printf 'export XDG_CONFIG_HOME=%q\n' "$XDG_CONFIG_HOME"
    else printf 'unset XDG_CONFIG_HOME\n'; fi
    [[ -n ${NESTED_PIDFILE:-} ]] && printf 'echo $$ > %q\n' "$NESTED_PIDFILE"
    printf 'exec'; printf ' %q' "$bin" "$@"; printf '\n'
} > "$wrapper"
chmod +x "$wrapper"

XDG_CONFIG_HOME=$kwin_cfg kwin_wayland --width 1600 --height 900 --output-count "$outputs" \
    --no-lockscreen --no-global-shortcuts --exit-with-session "$wrapper" &
kwin_pid=$!
wait "$kwin_pid" || true
kwin_pid=
