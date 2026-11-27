# Phase 1b: Bar

Read `00-overview.md` and `02-phase1-foundation.md` (Interfaces) first.

**Reference:** `~/caelestia/shell/modules/bar/` (`Bar.qml`, `BarWrapper.qml`,
`components/*`), defaults in `plugin/src/Caelestia/Config/barconfig.hpp`.

**Phase 1 widgets:** OS icon, workspaces, active window title, clock, tray,
status icons (audio, network, Bluetooth, battery), power button. Every other
Caelestia bar widget (media, weather, performance, GitHub, notes, keyboard
layout, keep-awake, screen capture, updates…) is phase 2, in
`06-phase2-daily-panels.md`.

**Pattern for every service (fixed here, used by all later tasks):**

```rust
// crates/caelum-services/src/lib.rs
pub trait Service: Send + 'static {
    type State: Clone + Send + 'static;
    /// Runs until the shell exits. Sends a fresh State on every change.
    /// Err(Unavailable) once at start if the backing service or hardware is absent.
    fn run(self, tx: tokio::sync::watch::Sender<Option<Self::State>>)
        -> impl std::future::Future<Output = Result<(), ServiceError>> + Send;
}
pub enum ServiceError { Unavailable(String), Failed(String) }
```

`caelum-shell` spawns each service on one shared tokio runtime (a
`current_thread` runtime on one background thread, to keep the thread count
low). It forwards `watch` changes to Slint with `slint::invoke_from_event_loop`.
`None` means the module is hidden.

---

### Task 03-1: Bar window and layout

**Files:**
- Create: `crates/caelum-ui/ui/bar/{bar.slint,bar-icon.slint}`, `crates/caelum-shell/src/bar.rs`
- Modify: `crates/caelum-config/src/model.rs` (add the `BarConfig` fields Phase 1 widgets use, defaults from `barconfig.hpp`: `workspaces.shown` = 5, `workspaces.active_indicator` = true, `workspaces.occupied_bg` = false, `workspaces.show_unoccupied` = true, `workspaces.show_windows` = true, and one `bool` per widget: `clock`, `tray`, `status_icons`, `workspaces`, `appmenu` = true)

**Interfaces:**
- Produces: `export component Bar` with `in property <Edge> position`, `in property <length> thickness`, and slots (child elements) for start/centre/end widget groups, matching Caelestia's `BarPlacements.qml`. `bar.rs`: `fn create_bar(output_id: u32, cfg: &Config) -> LayerWindow`. The bar is drawn **inside the Border window from 02-5** if the ADR route supports several Slint components in one window, otherwise as its own Top layer window anchored to `cfg.bar.position`. Record the choice in a comment at the top of `bar.rs`.
- [ ] **Step 1:** Implement the empty bar (no widgets) on its edge, with its exclusive zone set to the bar thickness, joined seamlessly to the border frame the way Caelestia's is.
- [ ] **Step 2: Verify:** nested KWin with 2 outputs. A maximised `konsole` sits beside the bar on both outputs, and `bar.position = "left"` in the config moves it live. Parity screenshot `docs/parity/03-1-bar.png`.
- [ ] **Step 3: Commit:** `git commit -m "Bar window joined to the border"`

### Task 03-2: Workspaces and active window (KDE protocols)

**Files:**
- Create: `crates/caelum-services/src/kwin/{mod.rs,state.rs}`, `crates/caelum-services/tests/kwin_state.rs`, `crates/caelum-ui/ui/bar/{workspaces.slint,active-window.slint}`

**Interfaces:**
- Consumes: `org_kde_plasma_virtual_desktop_management` and `org_kde_plasma_window_management` (from `caelum-wayland/protocols`). Reference behaviour: `~/caelestia/shell/services/Kwin.qml`, plus its `KWinWorkspaceState` and `KWinActiveWindowBridge` C++.
- Produces: `pub struct KwinState { pub workspaces: Vec<Workspace>, pub active_by_output: HashMap<String, String>, pub active_window: Option<WindowInfo> }`, with `Workspace { id: String, name: String, position: u32, occupied: bool }` and `WindowInfo { id: String, title: String, app_id: String, output: Option<String> }`. Plus `pub struct KwinTracker` (pure, fed by protocol events), `fn apply(&mut self, ev: KwinEvent) -> KwinState`, and `pub struct KwinService: Service<State = KwinState>`. Activating a workspace: `pub async fn activate_workspace(id: &str)`, through the protocol's `request_activate`.
- [ ] **Step 1: Write failing tests** for `KwinTracker`:
  - `desktops_sorted_by_position`: adding desktops at positions 2, 0, 1 yields `workspaces` ordered 0, 1, 2
  - `occupied_follows_windows`: a window on desktop B makes B `occupied`, and closing it clears it
  - `active_window_tracks_focus`: two windows, focus the second, so `active_window.title` is the second's
- [ ] **Step 2:** Tests fail. **Step 3:** Implement the tracker, then the Wayland event pump into it.
- [ ] **Step 4:** Tests pass. Nested check: creating desktops in the nested KWin (`qdbus org.kde.KWin /VirtualDesktopManager createDesktop 1 two`, pointed at the nested session's D-Bus) shows them. Clicking one switches desktop. The widget follows `shown`, `active_indicator`, `occupied_bg` and `show_unoccupied`.
- [ ] **Step 5: Commit:** `git commit -m "Workspaces and active window from KWin protocols"`

### Task 03-3: Clock and OS icon

**Files:**
- Create: `crates/caelum-ui/ui/bar/{clock.slint,os-icon.slint}`, `crates/caelum-shell/src/clock.rs`, `crates/caelum-ui/fonts/` (see below)

**Interfaces:**
- Produces: `clock.rs` runs a `slint::Timer` that re-arms itself to fire at the **next minute boundary** (or second boundary if the format has seconds), not every 1s. Format: copy Caelestia's default from `Clock.qml` / `generalconfig.hpp`, using `chrono` formatting.
- Icons: Caelestia uses the Material Symbols Rounded variable font (63MB mapped). Ship a **subset** containing only the glyphs Caelum uses: `tools/subset-icons.sh` runs `pyftsubset` with the codepoint list in `crates/caelum-ui/fonts/icons.txt`, and Slint embeds the subset with `import "fonts/material-symbols-subset.ttf"`. Each task that adds an icon adds its name to `icons.txt`.
- [ ] **Step 1:** Write a test `next_tick_is_minute_boundary`: for 12:00:37.250, `ms_until_next_tick(.., with_seconds=false)` == 22_750.
- [ ] **Step 2–4:** Fail, implement `pub fn ms_until_next_tick(now: chrono::NaiveTime, with_seconds: bool) -> u64`, pass.
- [ ] **Step 5:** OS icon from `/etc/os-release` `LOGO` / `ID`, matching Caelestia's `OsIcon.qml`. Parity screenshot. **Commit:** `git commit -m "Clock, OS icon, subset icon font"`

### Task 03-4: Tray (StatusNotifierItem host)

**Files:**
- Create: `crates/caelum-services/src/tray/{mod.rs,watcher.rs,item.rs,menu.rs}`, `crates/caelum-ui/ui/bar/tray.slint`, `crates/caelum-services/tests/tray.rs`

**Interfaces:**
- Produces: `TrayState { items: Vec<TrayItem> }`, with `TrayItem { service: String, id: String, title: String, icon: TrayIcon, status: Status }` and `enum TrayIcon { Name(String), Pixmap { w: u32, h: u32, argb: Arc<[u8]> } }`. `TrayService` registers as `org.kde.StatusNotifierWatcher` if no watcher exists (Plasma provides one, so normally it's a host on the existing watcher). `async fn activate(service, x, y)`, `async fn secondary_activate`, and `async fn menu(service) -> Menu` via `com.canonical.dbusmenu`. Icon names resolve through `freedesktop-icons`, **pixmaps are scaled to the bar icon size once and cached per item**, never kept at full size.
- [ ] **Step 1: Write failing tests** using a fake SNI item on a private `zbus` session (`zbus::connection::Builder::session` with a test-only bus via `dbus-daemon --session --print-address` started in the test):
  - `item_appears_and_disappears`: register a fake item, so `items.len()` == 1; drop its connection, so 0
  - `pixmap_is_downscaled`: a fake 256×256 pixmap is stored at the bar icon size
- [ ] **Step 2–4:** Fail, implement, pass. Nested check with `nm-applet` or any tray app: the icon shows, a left click activates it, a right click shows the menu (a Slint popup styled like Caelestia's tray menu).
- [ ] **Step 5: Commit:** `git commit -m "System tray host"`

### Task 03-5: Status icons (audio, network, Bluetooth, battery)

**Files:**
- Create: `crates/caelum-services/src/{audio.rs,network.rs,bluetooth.rs,battery.rs}`, `crates/caelum-services/tests/status.rs`, `crates/caelum-ui/ui/bar/status-icons.slint`

**Interfaces:**
- Produces (each one is a `Service`):
  - `AudioState { volume: f32, muted: bool, mic_volume: f32, mic_muted: bool, default_sink: String }`, from PipeWire through `libpulse-binding` (PipeWire's Pulse server), which subscribes to sink and source events. Also `async fn set_volume(f32)`, `async fn toggle_mute()`.
  - `NetworkState { kind: Wired|Wifi|None, ssid: Option<String>, strength: u8, vpn: bool }`, from NetworkManager D-Bus.
  - `BluetoothState { powered: bool, connected: Vec<String> }`, from BlueZ D-Bus, `Unavailable` with no adapter.
  - `BatteryState { percent: f32, charging: bool, time_to_empty_s: Option<u64> }`, from UPower's DisplayDevice, `Unavailable` when `IsPresent` is false (a desktop PC).
- Icons and thresholds copied from Caelestia's `StatusIcon.qml` and its services (`Audio.qml`, `Nmcli.qml`, `WifiSignal.qml`, `Connectivity.qml`).
- [ ] **Step 1: Write failing tests**, using a private dbus-daemon and fake services as in 03-4:
  - `absent_service_hides_module`: with no `org.freedesktop.UPower` on the bus, the battery watch value stays `None` and `run` returns `Err(Unavailable)`. The same applies to BlueZ.
  - `battery_present_reports_percent`: a fake UPower DisplayDevice at 42% gives `percent` == 42.0
  - `wifi_icon_buckets_match_caelestia`: `wifi_icon(strength)` for 0, 30, 60, 90 returns the same icon names as Caelestia's buckets
- [ ] **Step 2–4:** Fail, implement, pass. Real-desktop check (no switch needed): `cargo run -p caelum-services --example status` prints live states as you mute and unmute.
- [ ] **Step 5:** Widgets in `status-icons.slint`. Scrolling on the audio icon changes the volume, as in Caelestia. **Commit:** `git commit -m "Status icons: audio, network, bluetooth, battery"`

### Task 03-6: Bar memory check

- [ ] **Step 1:** `tools/memcheck.sh target/release/caelum-shell` with a tray app running in the nested session. Append the row to `docs/memory.md`.
- [ ] **Step 2: Gate:** VmRSS ≤ 200MB at this point, leaving room for 04 and 05 inside the 300MB budget. If it's over, find the biggest mappings (memcheck's stderr) before continuing, and record the cause in `docs/memory.md`.
- [ ] **Step 3: Commit:** `git commit -m "Record bar memory"`
