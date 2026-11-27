# Caelum Implementation Plan: Overview

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rebuild the Caelestia KDE shell in Rust and Slint with the same look,
using at most 300MB RSS, plus a Sukuna-themed startup splash, login screen and
boot animation.

**Architecture:** A Cargo workspace. `caelum-shell` runs one Slint event loop
and draws onto wlr-layer-shell surfaces on KWin, through the backend that
Phase 0 chooses. Async system services (zbus D-Bus, PipeWire, KDE Wayland
protocols) feed Slint models through `slint::invoke_from_event_loop`. Colours
come from the wallpaper through `material-colors`.

**Tech Stack:** Rust (stable), Slint, smithay-client-toolkit, wayland-client,
zbus, tokio, material-colors, serde/toml, notify. Login: SDDM QML. Boot:
Plymouth script theme.

**Spec:** `docs/superpowers/specs/2026-11-27-caelum-design.md`

**Reference codebase:** `~/caelestia` (the Caelestia shell, GPL-3.0). Its
behaviour and values are the source of truth for parity.

## Plan files (execute in order; 08 and 09 may run any time after 02)

| File | Phase | Detail level |
|---|---|---|
| `01-phase0-spike.md` | 0: layer-shell spike + measurement tools | Full |
| `02-phase1-foundation.md` | 1: workspace, config, theme, surfaces, unit | Full |
| `03-phase1-bar.md` | 1: bar and its modules | Full |
| `04-phase1-launcher-notifs-osd.md` | 1: launcher, notifications, OSD | Full |
| `05-phase1-splash-session.md` | 1: startup splash, session menu, switch-over | Full |
| `06-phase2-daily-panels.md` | 2: dashboard, sidebar, dock, lock | Task-level |
| `07-phase3-big-panels.md` | 3: Nexus, wallpaper picker, overview, screenshot | Task-level |
| `08-phase4-login.md` | 4: SDDM theme (L1) | Full |
| `09-phase5-boot.md` | 5: Plymouth (+ optional GRUB) | Full |

Nested KWin: `kwin_wayland --width W --height H [--output-count N] --no-lockscreen --no-global-shortcuts --exit-with-session <one executable path>`. The session flag takes a single path, so `tools/nested.sh` writes its arguments into a temporary wrapper script.

Phases 2 and 3 are deliberately task-level only. Their interfaces depend on
the backend Phase 0 picks and on the patterns Phase 1 settles. **Before
starting phase 2, re-run superpowers:writing-plans on file 06** (and the
same for 07 before phase 3) to expand it to full detail.

## Global Constraints

- Licence: GPL-3.0-or-later. A `LICENSE` file and an SPDX header in every source file.
- Memory budget: ≤300MB VmRSS after 60s idle with all Phase 1 features on 2 screens; target ~150MB. Measured with `tools/memcheck.sh`; each phase appends a row to `docs/memory.md`.
- Compositor: KWin Wayland only. Protocol XML copied from `~/caelestia/shell/plugin/src/Caelestia/protocols/`.
- Never commit wallpapers. The default is the path `~/Pictures/wallpaper/anime/sukuna-hoodie-3840x2160-26053.jpg`; rotation adds `sukuna-action-3840x2160-25100.jpg` and `sukuna and raga.jpeg` from the same folder.
- Config: `~/.config/caelum/config.toml` (respect `$XDG_CONFIG_HOME`).
- Design tokens are copied from Caelestia, not invented: defaults in `~/caelestia/shell/plugin/src/Caelestia/Config/*.hpp` (`CONFIG_PROPERTY(type, name, default)`), animation curves in `Config/anim.cpp`. Example: border `thickness` 10, `rounding` 25.
- Never disturb the running Caelestia desktop while developing: test in Slint preview or nested KWin (`tools/nested.sh`).
- The user unit is `caelum-shell.service` with `Conflicts=caelestia-shell.service` and `MemoryTHP=madvise`.
- No `unwrap()`/`expect()` outside tests and `main` startup; a missing service or file degrades a module, never panics the shell.
- Every crate passes `cargo clippy -- -D warnings` and `cargo fmt --check`.

## Review Focus

1. **Monitor hotplug:** unplugging or plugging in a screen removes or adds that screen's surfaces, with no crash and no leftover reserved edge. Test: `caelum-wayland` task 02-4 `outputs_removed_drops_surfaces`.
2. **Wallpaper missing or unreadable:** the shell still starts, using `Scheme::fallback()`, and logs a warning. Test: task 02-3 `missing_wallpaper_uses_fallback`.
3. **Notification flood:** 500 notifications in a second keeps at most `notifs.max_popups` popups and caps history at `notifs.max_history`, so memory stays bounded. Test: task 04-3 `flood_is_capped`.
4. **Hardware or service absent** (no battery, no Bluetooth adapter, NetworkManager down): that module hides itself and nothing panics. Test: task 03-5 `absent_service_hides_module`.
5. **Broken config during live reload:** the last good config is kept and the error is logged. Test: task 02-2 `invalid_reload_keeps_previous`.

## Which Claude model to build with

- **Sonnet 5.5 for most tasks.** The plan already makes the design
  decisions, and Sonnet handles well-specified Rust tasks well.
- **Opus 5.5 for Phase 0** (the Slint layer-shell backend is the hardest
  unknown), for any bug that survives two attempts, and for the expansion
  passes on files 06 and 07.
- **Haiku 5.5 for mechanical chores:** copying token values, renaming,
  writing README rollback docs.

To keep token use low: run **Native** execution rather than subagent-driven,
start a fresh session per plan file (`/clear` between files), and switch
models with `/model`.
