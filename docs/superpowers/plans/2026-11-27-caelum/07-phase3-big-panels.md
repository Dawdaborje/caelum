# Phase 3: Big Panels (task-level)

> **Expand before executing:** run superpowers:writing-plans on this file
> once Phase 2 is accepted. Use Opus for that expansion: Nexus and the
> overview are the largest Caelestia modules.

**Reference:** `~/caelestia/shell/modules/{nexus,overview,screenshot,windowinfo,background}/`,
`services/Wallpapers.qml`, `plugin/src/Caelestia/Images/` (thumbnail and
analysis code, including the 2026-11-27 decode-size fixes).

| Task | Builds | Key tests |
|---|---|---|
| 07-1 Wallpaper service | Scan dirs from config (default `~/Pictures/wallpaper/anime`), rotation including the 3 Sukuna images, thumbnails **decoded at their target size** into `~/.cache/caelum/thumbs/` (keyed by path + mtime) | `thumb_decoded_at_target_size` (a 3840×2160 fixture gives a cached ≤ 400px thumbnail); `cache_hit_skips_decode`; rotation order is stable |
| 07-2 Wallpaper picker | The grid from Caelestia's wallpaper select, colour sort using `scheme_from_image` results **computed once and cached** (the Caelestia re-sort bug fixed on 2026-11-27) | sorting is stable across repeated calls; picking one applies it and recomputes the scheme |
| 07-3 Nexus settings app | A normal `xdg_toplevel` Slint window (not layer-shell): navigation pane, pages that write `config.toml` (live reload does the rest). Port pages in order: Appearance, Bar, Wallpaper & Style, Notifications, Launcher, then the rest | each page round-trips: load, change, save, reload equal; unknown keys in the file are preserved on save |
| 07-4 Overview | Workspace grid with window previews. Previews come from KWin's screencast protocol (`zkde-screencast-unstable-v1`, in Caelestia's protocol list), **only while the overview is open**, with every stream closed on close | open then close leaves 0 active streams; RSS back within 10MB of before |
| 07-5 Screenshot + recorder | Region selector (Overlay surface), capture through KWin's ScreenShot2 D-Bus, and a recorder as in `modules/screenshot/recorder` | region maths (`normalize_rect` for drags in every direction) |
| 07-6 Window info | `modules/windowinfo` details and preview | — (parity check only) |
| 07-7 Phase 3 memory gate | memcheck row with the overview closed; ≤300MB | — |

Out of scope even here (from the spec's non-goals): plugins, AI assistant,
Discord RPC, QuickShare, Wallhaven, Dino game, translations. Re-decide after
Phase 3.
