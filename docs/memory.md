# Memory

One row per measurement, from `just mem <binary>` (`tools/memcheck.sh`):
VmRSS after 60s idle in nested KWin with 2 outputs. Budget ≤ 300000 kB, target ~150000 kB.

| Date | Build | VmRSS kB | RssAnon kB | AnonHugePages kB | Threads |
|---|---|---|---|---|---|
| 2026-11-27 | caelestia c90c40f | 780040 | ~460000 | 0 | 13 |
| 2026-11-27 | spike route 3 (software, 2 bars) | 40504 | 10892 | 0 | 1 |
