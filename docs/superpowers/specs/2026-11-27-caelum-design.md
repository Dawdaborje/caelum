# Caelum: design

Date: 2026-11-27
Status: draft, waiting for approval

## Intent

Caelum is a Rust rewrite of the Caelestia KDE shell. It keeps the **same design
and behaviour** but uses **much less memory**. Caelestia on Quickshell/QML sits
at about 780MB RSS even after the hugepage fix. An empty Quickshell process is
168MB, so about 610MB of that comes from Caelestia's QML content and data,
which is the part a compiled UI can shrink.

The project also covers the rest of the visual chain from power-on to desktop:
the **boot animation**, the **login screen** and the **startup splash**. The
whole thing is themed around the owner's **Sukuna** wallpapers.

Caelestia stays the daily driver while Caelum is built. The owner keeps
maintaining Caelestia in parallel for the coming months.

## Goals

1. Visual and behavioural parity with Caelestia: the bar joined to the screen
   border, slide-out drawers, Material You colours from the wallpaper, rounded
   animated panels.
2. Idle memory **at or below 300MB RSS** with the first milestone's features,
   measured the way the Caelestia investigation measured it (VmRSS after 60s
   idle, plus a `smaps_rollup` breakdown). Aim for about 150MB.
3. Everything can be developed and tested **without touching the running
   Caelestia desktop** (see Testing).
4. Switching between Caelestia and Caelum is one command and fully reversible.
5. A Sukuna-themed boot animation, login screen and startup splash that match
   the shell's look.

## Non-goals (for now)

- Writing a compositor or window manager. Caelum runs on **KWin** in the
  existing Plasma session, the same place Caelestia runs. A Smithay compositor
  stays a possible later project and is out of scope here.
- Hyprland support. Caelestia has some; Caelum targets KWin only until parity.
- Migrating existing Caelestia config files. Caelum has its own config.
- Caelestia's plugin system and the AI, Discord RPC, QuickShare and Wallhaven
  services, until after parity with the core shell.
- Translations (Crowdin) until after parity.

## Anti-goals (failure even if it works)

- Caelum ends up using as much memory as Caelestia. Every milestone is
  measured against the budget, and a milestone that blows it is not done.
- It looks "close enough". If Caelum looks noticeably different from
  Caelestia, the rewrite has failed its point.
- Building it breaks or disturbs the owner's working Caelestia setup.
- A broken boot or login screen leaves the machine hard to recover. Every boot
  and login change has a documented one-command rollback.

## Constraints

- Machine: Arch Linux, KDE Plasma 6, KWin Wayland, AMD Lucienne APU (radeonsi),
  two monitors, GRUB, systemd 262, kernel THP in `always` mode.
- Caelestia is GPL-3.0-or-later. Caelum reimplements its design. Any code,
  shaders or assets copied or closely translated from Caelestia make Caelum a
  derivative, so **Caelum is GPL-3.0-or-later** too.
- **Wallpapers are not committed.** The Sukuna images are fan art the owner
  downloaded, so they are referenced by path, never copied into the repo:
  - `~/Pictures/wallpaper/anime/sukuna-hoodie-3840x2160-26053.jpg` (default)
  - `~/Pictures/wallpaper/anime/sukuna-action-3840x2160-25100.jpg`
  - `~/Pictures/wallpaper/anime/sukuna and raga.jpeg`
- KWin is the compositor, so window and workspace data has to come from the
  protocols Caelestia already uses: `plasma-window-management`,
  `org-kde-plasma-virtual-desktop` and `kde-output-device-v2`. Their XML is in
  `caelestia/shell/plugin/src/Caelestia/protocols/`. The reference behaviour is
  `caelestia/shell/services/Kwin.qml` and its C++ bridges.
- SDDM loads only QML themes, Plymouth only its own script or image themes, and
  KSplash only Plasma look-and-feel QML. Slint can't plug into any of the three
  directly (see the login and boot sections).

## Approaches considered

| Approach | Trade-off | Chosen? |
|---|---|---|
| **A. Rust and Slint rewrite** | About 150–300MB estimated. Slint is declarative like QML, so panels port nearly one-to-one, and live preview works without the desktop. Many months of work. | **Yes.** The owner picked it. |
| B. Slim the current QML shell | About 500–600MB estimated in days, with nothing to rebuild. It can't reach the target, because Qt and the QML engine stay. | No. The owner keeps Caelestia running in parallel instead. |
| C. B first, then A | Lower risk, and the decision rests on measured numbers. | No. The owner went straight to A. |
| Rust with iced | Mature and used by COSMIC, but the Elm model is further from QML, so porting is slower. | No. Slint fits a QML port better. |
| Own compositor (Smithay) | Full control, but a multi-year project. | No, out of scope (see non-goals). |

## Architecture

A single Rust workspace in this repo:

```
caelum/
  crates/
    caelum-shell/      binary: owns the event loop, windows, startup splash
    caelum-ui/         .slint files: components, panels, theme tokens
    caelum-wayland/    layer-shell surfaces + KDE protocols (wayland-client)
    caelum-services/   D-Bus and system services (zbus, async)
    caelum-theme/      wallpaper -> Material You scheme (material-colors crate)
    caelum-config/     config file loading and live reload (serde + toml)
  login/               login screen (see Login screen)
  boot/                Plymouth theme + tools to build it
  tools/               memory measurement script, nested-KWin launcher
```

### UI and windowing: the biggest technical risk

The shell needs `wlr-layer-shell` surfaces (bar, drawers, exclusion zones,
overlays). Slint's stock winit backend can't create those. Options, cheapest
first:

1. An existing community crate that runs Slint on layer-shell, if one is
   maintained and works on KWin.
2. A small custom Slint platform backend in `caelum-wayland`, using
   `smithay-client-toolkit` for layer-shell and Slint's FemtoVG (OpenGL)
   renderer, or its software renderer for tiny surfaces.

Phase 0 settles this before anything else gets built.

**Windows: fewer than Caelestia.** Caelestia uses about 13 windows across two
screens, four of them invisible 1×1 exclusion windows per screen. Caelum
should use one content surface per screen plus the smallest set of exclusion
surfaces the layout needs. One renderer and one GL context should be shared
across surfaces wherever the backend allows.

### Services (`caelum-services`)

These are Rust ports of Caelestia's services, each one talking to the system
directly:

| Need | Source |
|---|---|
| Notifications (being the server) | `org.freedesktop.Notifications` via zbus |
| Media | MPRIS over D-Bus |
| Battery | UPower |
| Network, Wi-Fi, VPN | NetworkManager D-Bus |
| Bluetooth | BlueZ D-Bus |
| Audio volume and devices | PipeWire (`pipewire` crate) or the PulseAudio API |
| Brightness | logind `SetBrightness` / backlight sysfs |
| Tray | StatusNotifierItem host over D-Bus |
| Workspaces and windows | KDE Wayland protocols (above) |
| App list for the launcher | XDG `.desktop` files (`freedesktop-desktop-entry`) |
| Calculator | a Rust expression crate instead of libqalculate, unless parity needs qalc's units and currency |

### Theme

`caelum-theme` generates the Material You scheme from the current wallpaper,
the same way Caelestia does through matugen. Theme tokens (colours, radii,
spacing, animation curves and durations) live in one Slint global, so every
panel uses the same values. Caelestia's token values in its QML config are
the reference, and they get copied over, not re-invented.

The default wallpaper is the **Sukuna hoodie** image. The scheme generated from
it seeds the shell, the startup splash and the login screen. The boot
animation uses fixed colours taken from the same scheme at build time.

### Config

Lives in `~/.config/caelum/config.toml` and reloads live when the file changes.
It only holds options that the implemented features use.

## Startup splash

KSplash is disabled on this machine (`Engine=none`), so Caelum's splash is
part of the shell itself. On start, `caelum-shell` shows a full-screen overlay
on each screen: the Sukuna wallpaper, a logo and a short animation. It fades
out as soon as the bar and the wallpaper have drawn their first frame. Writing
it in Rust/Slint means no extra process and no QML.

## Login screen (owner to decide)

SDDM greeters are QML-only, and the greeter process exits after login, so its
memory doesn't affect the desktop. Two options:

- **L1, recommended: restyle the existing Caelestia SDDM theme**
  (`caelestia/src/sddm/themes/full`, about 4,700 lines of QML) with Sukuna
  and Caelum's colours. This is the cheapest route, and the memory goal doesn't
  apply to it.
- **L2: switch to greetd plus a Rust/Slint greeter.** Everything is Rust, and
  the greeter reuses the shell's components. But it replaces SDDM, means
  writing PAM-backed login, session picking, multi-monitor and power actions,
  and a bug here locks the owner out of the machine.

Also: `/etc/sddm.conf.d/` has several files that each set a different
`Current=` theme (material-you, panacea, sddm-astronaut-theme, caelestia).
Whichever option is chosen, that gets consolidated into one file.

## Boot animation (owner to decide on scope)

Plymouth isn't installed. The machine boots GRUB, then the kernel with
`quiet`. Plymouth themes are a script plus images, not Rust. The plan is:

- Install Plymouth, add its hook in mkinitcpio, add `splash` to the GRUB
  kernel command line, and regenerate the initramfs and `grub.cfg`.
- A `caelum` Plymouth theme: a background taken from the Sukuna wallpaper
  (resized and darkened), a logo, and a looping animation. The frames are
  pre-rendered by a small Rust tool in `boot/` from the scheme, so they match
  the shell's colours. No password prompt is needed: the disk isn't encrypted
  (checked with `lsblk`).
- Rollback: `plymouth-set-default-theme -R bgrt`, or remove `splash` from the
  command line. Both are documented in `boot/README.md`.
- Optional: a matching GRUB menu theme. The owner decides whether
  "boot loading animation" includes the GRUB menu.

## Testing without touching Caelestia

1. **Component work:** Slint live preview, or ordinary windows (`cargo run
   --example bar`).
2. **Whole shell:** a nested KWin in a window, started by
   `tools/nested.sh`, which runs
   `kwin_wayland --width 1600 --height 900 --exit-with-session caelum-shell`.
3. **Daily use:** a `caelum-shell.service` systemd user unit with
   `Conflicts=caelestia-shell.service` and `MemoryTHP=madvise`. Starting it
   stops Caelestia, and `systemctl --user start caelestia-shell` switches
   back.
4. **Login and boot:** the SDDM test mode
   (`sddm-greeter-qt6 --test-mode --theme ...`) for L1. For boot,
   `plymouthd; plymouth --show-splash` in a VM or on a TTY before installing.

**Memory check:** `tools/memcheck.sh` starts Caelum nested, waits 60s, then
prints VmRSS, RssAnon, AnonHugePages and the top mappings. Every milestone
records its number in `docs/memory.md`.

## Phases

| Phase | Delivers | Done when |
|---|---|---|
| 0. Spike (throwaway) | One Slint bar as a layer-shell surface on nested KWin, with a clock and colours from the Sukuna wallpaper | It renders and reserves its edge on KWin, and its RSS is measured. **Go/no-go:** an empty bar above about 120MB means rethinking the renderer before continuing. |
| 1. First switchable shell | Bar (workspaces, clock, tray, audio, network, battery, Bluetooth), launcher (apps + calculator), notification server + popups, OSDs (volume, brightness), wallpaper + scheme, startup splash, session menu | The owner can use it all day in place of Caelestia, matching its look, under 300MB |
| 2. Daily panels | Dashboard (media, calendar, weather, performance), sidebar, dock, lock screen integration | Parity for everyday use |
| 3. Big panels | Nexus settings, wallpaper picker, overview, screenshot and recorder, window info | Full parity with Caelestia's core |
| 4. Login screen | Option L1 or L2 | Logs in reliably, rollback tested |
| 5. Boot animation | Plymouth theme (+ GRUB theme if wanted) | Boots with it, rollback tested |

Phases 4 and 5 don't depend on the shell, so they can run in parallel with
phases 2 and 3 whenever the owner wants a break from shell work.

## Calls made in this design (owner may override)

- First milestone scope: bar, launcher, notifications, OSDs, wallpaper and
  scheme, splash, session menu. The owner was asked but didn't pick, so this
  is the smallest set that replaces Caelestia day to day.
- KWin stays the compositor. There's no new session entry: switching happens
  through the user unit's `Conflicts=`.
- Default wallpaper: the Sukuna hoodie image. The other two Sukuna images go
  in the wallpaper rotation and picker.
- Caelum is GPL-3.0-or-later.
- Lock screen: Caelestia uses Plasma's lock screen (QML, kscreenlocker).
  Caelum keeps using it in phases 1–2 and restyles it with Sukuna later. Unverified: whether KWin
  supports `ext-session-lock` for a Rust lock screen. Phase 2 checks.
- Calculator without libqalculate unless parity shows it's needed.

## Left to the builder

- Choosing the Slint layer-shell route, through the Phase 0 spike.
- Crate versions, async runtime (tokio or smol, whichever zbus and
  Slint's event loop fit best), logging.
- Exact token values, copied from Caelestia's config and QML.
- How the D-Bus services feed Slint models (Slint's `invoke_from_event_loop`
  or a channel).
