# Phase 2: Daily Panels (task-level)

> **Expand before executing:** run superpowers:writing-plans on this file
> once Phase 1 is accepted. The expansion turns each task below into full
> Files / Interfaces / test steps, using the patterns Phase 1 settled
> (`Service`, `Drawer`, `LayerWindow`, the parity screenshot convention).

Read `00-overview.md` and `docs/parity/gaps.md` (from 05-3) first. Non-blocker
gaps from Phase 1 get folded into the matching task below.

**Reference:** `~/caelestia/shell/modules/{dashboard,sidebar,dock,utilities}/`
and the bar widgets listed in `03-phase1-bar.md` as phase 2.

| Task | Builds | Key tests (each one is a pure-logic unit test, plus a nested parity check) |
|---|---|---|
| 06-1 Media service + bar widget | MPRIS `Service`, `PlayersState { active, players }`, bar media widget | active-player selection when 2 players play; `Unavailable` with no player shows nothing |
| 06-2 Dashboard drawer | `Dash.qml`, `Media.qml`, `Performance.qml` and `WeatherTab.qml` tabs, using `Drawer` from 04-2 | tab switching keeps state; weather caches its last result and shows it offline |
| 06-3 Weather + performance services | Weather via Open-Meteo (Caelestia's `Weather.qml` source), with the location from config (Caelestia's ipinfo auto-location hits rate limits, as seen in its logs); CPU, RAM and GPU from `/proc` and `sysfs` | `parse_open_meteo(fixture)`; `cpu_percent(prev, now)` from two `/proc/stat` fixtures |
| 06-4 Sidebar + notification history | `NotifDock`, groups, swipe to dismiss; reads `NotifStore::history()` | grouping by app, with `group_preview_num` = 3; clear-all empties history |
| 06-5 Dock | `modules/dock/` behaviour: pinned apps plus running windows from `KwinState` | pinned and running merge without duplicates |
| 06-6 Remaining bar widgets | keyboard layout, keep-awake (logind inhibitor), notifications indicator, show desktop, app menu, screen-capture indicator | one logic test per widget that has logic |
| 06-7 Utilities: toasts and modals | `modules/utilities/toasts`, confirm modal | toast queue caps and expiry |
| 06-8 Lock screen restyle | Restyle Plasma's lock screen with Sukuna, keeping kscreenlocker. Copy and adapt `~/caelestia/src/kde/shells/caelestia.desktop/contents/lockscreen/` into `lock/` and install it the same way Caelestia's installer does (read `install.sh` / `scripts/` for the path). **Also check:** `wayland-info \| grep ext_session_lock` on KWin, and record whether a Rust lock screen is possible later in `docs/adr/0002-lock.md` | `loginctl lock-session` shows the restyled lock screen; unlocking works; rollback documented |
| 06-9 Phase 2 memory gate | memcheck row; ≤300MB still holds | — |
