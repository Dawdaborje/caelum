# Memory

One row per measurement, from `just mem <binary>` (`tools/memcheck.sh`):
VmRSS after 60s idle in nested KWin with 2 outputs. Budget ≤ 300000 kB, target ~150000 kB.

| Date | Build | VmRSS kB | RssAnon kB | AnonHugePages kB | Threads |
|---|---|---|---|---|---|
| 2026-11-27 | caelestia c90c40f | 780040 | ~460000 | 0 | 13 |
| 2026-11-27 | spike route 3 (software, 2 bars) | 40504 | 10892 | 0 | 1 |
| 2026-11-28 | 02-5 wallpaper + border, 2 outputs | 72696 | 38832 | 0 | 1 |
| 2026-11-28 | 03 bar (tray with nm-applet, status icons), 2 outputs | 101744 | 40644 | 16384 | 5 |

03 bar: +29 MB over 02-5. Biggest mappings are heap 38 MB and the layer surfaces'
shm buffers 23 MB (`memfd:smithay-client-toolkit`: background, border and bar per
output), then fontconfig caches ~14 MB. The 16 MB of AnonHugePages is transparent
hugepages backing heap, since the nested run has no `MemoryTHP=madvise` (the user unit does).
Threads: main, tokio, pulse, and two from zbus.
