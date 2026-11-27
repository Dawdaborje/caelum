#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Start <binary> in nested KWin with 2 outputs, wait 60s, print a docs/memory.md row.
# Top 10 mappings by RSS go to stderr.
# Usage: tools/memcheck.sh <binary> [args...]
set -euo pipefail

[[ $# -ge 1 ]] || { echo "usage: $0 <binary> [args...]" >&2; exit 2; }
bin=$1
here=$(dirname "$(realpath "$0")")

# nested.sh writes the binary's own PID here, so we never pick up a same-named process.
pidfile=$(mktemp)
NESTED_PIDFILE=$pidfile "$here/nested.sh" --outputs 2 "$@" &
nested_pid=$!
trap 'kill "$nested_pid" 2>/dev/null; wait "$nested_pid" 2>/dev/null || true; rm -f "$pidfile"' EXIT

pid=
for _ in $(seq 50); do
    pid=$(<"$pidfile")
    [[ -n $pid ]] && break
    sleep 0.2
done
[[ -n $pid ]] || { echo "memcheck: $bin did not start" >&2; exit 1; }

sleep 60
kill -0 "$pid" 2>/dev/null || { echo "memcheck: $bin exited early" >&2; exit 1; }

field() { awk -v k="$1:" '$1 == k { print $2 }' "$2"; }
vmrss=$(field VmRSS "/proc/$pid/status")
rssanon=$(field RssAnon "/proc/$pid/status")
huge=$(field AnonHugePages "/proc/$pid/smaps_rollup")
# shortcut: counts all threads, not only render ones; split by name if it matters.
threads=$(field Threads "/proc/$pid/status")
sha=$(git -C "$here/.." describe --always --dirty 2>/dev/null || echo none)

# The path starts at the first / or [; it may contain spaces.
awk '/^[0-9a-f]+-[0-9a-f]+ / { name = match($0, /[\/[].*/) ? substr($0, RSTART) : "[anon]" }
     $1 == "Rss:" { rss[name] += $2 }
     END { for (n in rss) printf "%10d kB  %s\n", rss[n], n }' "/proc/$pid/smaps" \
    | sort -rn | head -10 >&2

printf '| %s | %s | %s | %s | %s | %s |\n' "$(date +%F)" "$sha" "$vmrss" "$rssanon" "$huge" "$threads"
