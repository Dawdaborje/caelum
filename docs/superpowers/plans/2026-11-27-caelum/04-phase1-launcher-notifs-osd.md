# Phase 1c: Shortcuts, Drawers, Launcher, Notifications, OSD

Read `00-overview.md`, `02-…` and `03-…` (the `Service` pattern) first.

**Reference:** `~/caelestia/shell/modules/{drawers,launcher,notifications,osd}/`,
`plugin/src/Caelestia/Services/globalshortcut.cpp`,
`Config/{launcherconfig,notifsconfig,osdconfig}.hpp`.

---

### Task 04-1: Shell D-Bus interface and global shortcuts

**Files:**
- Create: `crates/caelum-shell/src/{ipc.rs,shortcuts.rs}`, `crates/caelum-shell/tests/ipc.rs`, `crates/caelum-ctl/` (a tiny binary `caelum` added to the workspace)

**Interfaces:**
- Produces:
  - A D-Bus service `org.caelum.Shell` at `/org/caelum/Shell`, interface `org.caelum.Shell1`, with methods `Toggle(drawer: s)`, `Open(drawer: s)`, `Close(drawer: s)`, `Reload()`. Drawer names: `"launcher"`, `"session"`, and later `"dashboard"`, `"sidebar"`. An unknown name returns the D-Bus error `org.caelum.Shell1.Error.UnknownDrawer`.
  - `pub enum Drawer { Launcher, Session, Dashboard, Sidebar }` with `FromStr`. `ipc.rs` sends `DrawerCommand { drawer: Drawer, action: Toggle|Open|Close }` into the UI thread.
  - CLI: `caelum toggle launcher`, which calls the method above.
  - `shortcuts.rs` registers global shortcuts with `org.kde.kglobalaccel` (component `caelum`) the same way `globalshortcut.cpp` does, with Caelestia's default keys from `Config/keybindsdefaults.hpp`.
- [x] **Step 1: Write failing tests** (private dbus-daemon): `toggle_known_drawer_sends_command` (Toggle("launcher") gives one `DrawerCommand{Launcher, Toggle}` on the channel) and `unknown_drawer_is_dbus_error`.
- [x] **Step 2–4:** Fail, implement, pass.
- [x] **Step 5: Verify:** `caelum toggle launcher` in the nested session logs the command. **Commit:** `git commit -m "Shell D-Bus interface, CLI, global shortcuts"`

### Task 04-2: Drawer mechanics and launcher

**Files:**
- Create: `crates/caelum-ui/ui/drawers/drawer.slint`, `crates/caelum-ui/ui/launcher/{launcher.slint,app-item.slint}`, `crates/caelum-services/src/apps.rs`, `crates/caelum-services/src/calc.rs`, `crates/caelum-services/tests/{apps.rs,calc.rs}`

**Interfaces:**
- Produces:
  - `drawer.slint`: `export component Drawer` with `in property <bool> open`, `in property <Edge> edge`, whose content slides out of the border/bar using `Theme`'s Caelestia curves and durations (from `caelum-theme::tokens`, exposed in `theme.slint`), and whose shape joins the border with the same rounded "flare" corners Caelestia draws. Every drawer in later phases reuses this.
  - While a drawer is open, its window's input region covers the drawer and keyboard is `OnDemand` (`Exclusive` for the launcher). When it's closed, the input region is empty.
  - `apps.rs`: `pub struct AppEntry { id: String, name: String, generic: Option<String>, keywords: Vec<String>, icon: String, exec: String, terminal: bool }`, `pub fn load_apps() -> Vec<AppEntry>` (XDG data dirs via `freedesktop-desktop-entry`, `NoDisplay`/`Hidden` skipped, deduped by id), `pub fn search(apps: &[AppEntry], query: &str, limit: usize) -> Vec<&AppEntry>` (fuzzy via `nucleo-matcher`, frecency boost from `~/.local/state/caelum/frecency.json`), and `pub fn launch(&AppEntry)`, which starts the app **through `systemd-run --user --scope`** so it isn't a child of the shell (Caelestia issue #402: apps must not inherit the shell's stdio).
  - `calc.rs`: `pub fn evaluate(expr: &str) -> Option<String>` with the `meval` crate (or `evalexpr`). Input starting with `=` is a calculation, as in Caelestia.
  - The launcher shows at most `launcher.max_shown` = 7 results (from `launcherconfig.hpp`). Add `LauncherConfig { max_shown: usize = 7 }` and `OsdConfig { hide_delay_ms: u32 = 2000, enable_brightness: bool = true }` (the latter is used by 04-4) to `caelum-config`.
- [ ] **Step 1: Write failing tests:**
  - `hidden_entries_skipped`: a fixture dir with a `NoDisplay=true` desktop file gives 0 apps
  - `search_prefers_name_prefix`: apps "Firefox" and "Profile Manager", query "fi", puts Firefox first
  - `limit_respected`: 20 apps, `search(…, "", 7).len()` == 7
  - `calc_basic`: `evaluate("2+2*3")` == `Some("8")`, and `evaluate("2+")` == `None`
- [ ] **Step 2–4:** Fail, implement, pass.
- [ ] **Step 5: Verify:** nested. The shortcut or `caelum toggle launcher` slides the launcher out with Caelestia's animation. Typing filters, Enter launches, Esc closes. Parity: a screen recording of both shells opening the launcher, saved as `docs/parity/04-2-launcher.webm`. The timing and curve should look identical.
- [ ] **Step 6: Commit:** `git commit -m "Drawer mechanics and app launcher"`

### Task 04-3: Notification server and popups

**Files:**
- Create: `crates/caelum-services/src/notifs/{mod.rs,server.rs,store.rs}`, `crates/caelum-services/tests/notifs.rs`, `crates/caelum-ui/ui/notifications/{popup.slint,popup-list.slint}`
- Modify: `crates/caelum-config/src/model.rs`, adding `NotifsConfig { max_popups: usize = 5, max_history: usize = 200, default_timeout_ms: u32 = 5000, expand_threshold: u32 = 20, group_preview_num: u32 = 3, clear_threshold: f32 = 0.3 }`. `max_popups` and `max_history` are Caelum's own caps (Caelestia has none). The other three come from `notifsconfig.hpp`. Copy Caelestia's default timeout from its config or `Notifs.qml` if it differs from 5000.

**Interfaces:**
- Produces:
  - `server.rs`: implements `org.freedesktop.Notifications` (`Notify`, `CloseNotification`, `GetCapabilities` = `["actions","body","body-markup","icon-static","persistence"]`, `GetServerInformation` = `("caelum","caelum","0.1","1.2")`), emitting `NotificationClosed` and `ActionInvoked`. If the name is already taken (Caelestia running), it returns `Err(Unavailable)` and does not panic.
  - `store.rs`: `pub struct NotifStore` with `fn add(&mut self, n: Notification) -> u32`, `fn close(&mut self, id: u32, reason: CloseReason)`, `fn popups(&self) -> &[Notification]` (newest first, ≤ `max_popups`), `fn history(&self) -> impl Iterator<Item=&Notification>` (≤ `max_history`, oldest evicted). **Image hints (`image-data`) are downscaled to 64×64 on arrival.**
  - `Notification { id, app_name, app_icon, summary, body, actions: Vec<(String,String)>, urgency, timeout_ms, image: Option<Arc<[u8]>>, received: SystemTime }`
- [ ] **Step 1: Write failing tests:**
  - `notify_returns_increasing_ids`
  - `replaces_id_updates_in_place`: Notify with `replaces_id` set keeps one entry with the new summary
  - `flood_is_capped`: 500 `add`s give `popups().len()` ≤ 5 and `history().count()` == 200
  - `image_downscaled`: a 512×512 image hint is stored as 64×64
  - `name_taken_is_unavailable`: with another connection owning the name, the server returns `Unavailable`
- [ ] **Step 2–4:** Fail, implement, pass.
- [ ] **Step 5: Verify:** nested. `notify-send -a test "Hello" "World"` (pointed at the nested session's D-Bus) shows a popup styled like Caelestia's `Notification.qml`. It times out, can be swiped or clicked to dismiss, and actions work (`notify-send -A yes=Yes …`). **Commit:** `git commit -m "Notification server and popups"`

### Task 04-4: OSD (volume, brightness)

**Files:**
- Create: `crates/caelum-services/src/brightness.rs`, `crates/caelum-services/tests/brightness.rs`, `crates/caelum-ui/ui/osd/osd.slint`

**Interfaces:**
- Consumes: `AudioState` from 03-5
- Produces: `BrightnessState { percent: f32 }`, read from `/sys/class/backlight/*` and written through logind `org.freedesktop.login1.Session.SetBrightness`. It is `Unavailable` with no backlight (an external monitor: DDC/CI is out of scope in Phase 1). The OSD appears on change and hides after `osd.hide_delay_ms` = 2000. It shows brightness only if `osd.enable_brightness` = true (from `osdconfig.hpp`).
- [ ] **Step 1: Write failing tests:** `percent_from_sysfs`, reading fixture files `brightness`=50 and `max_brightness`=200 to give 25.0, and `no_backlight_unavailable`, an empty dir giving `Unavailable`.
- [ ] **Step 2–4:** Fail, implement, pass.
- [ ] **Step 5: Verify:** the volume keys (or `wpctl set-volume @DEFAULT_SINK@ 5%+`) show the OSD, which hides after 2s. Parity screenshot. **Commit:** `git commit -m "Volume and brightness OSD"`
