#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Switch the running shell between Caelum and Caelestia. Never enables either unit.
# Usage: tools/switch.sh caelum|caelestia
set -euo pipefail
cd "$(dirname "$0")/.."

case ${1:-} in
caelum)
    cargo build --release -p caelum-shell
    install -Dm755 target/release/caelum-shell ~/.local/bin/caelum-shell
    install -Dm644 dist/caelum-shell.service ~/.config/systemd/user/caelum-shell.service
    systemctl --user daemon-reload
    # Conflicts= stops Caelestia.
    systemctl --user start caelum-shell
    ;;
caelestia)
    # Conflicts= works in reverse, so this stops Caelum.
    systemctl --user start caelestia-shell
    ;;
*)
    echo "usage: $0 caelum|caelestia" >&2
    exit 2
    ;;
esac
