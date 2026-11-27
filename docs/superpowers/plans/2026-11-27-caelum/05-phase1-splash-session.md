# Phase 1d: Startup Splash, Session Menu, Switch-over

Read `00-overview.md` and files 02–04 (Interfaces) first.

**Reference:** `~/caelestia/shell/modules/session/`, `Config/sessionconfig.hpp`.

---

### Task 05-1: Startup splash

**Files:**
- Create: `crates/caelum-ui/ui/splash/splash.slint`, `crates/caelum-shell/src/splash.rs`, `crates/caelum-shell/tests/splash.rs`, `crates/caelum-ui/assets/caelum-logo.svg` (an original logo: a stylised constellation "C" in `Theme.primary`. No Caelestia or third-party logo)

**Interfaces:**
- Consumes: `create_layer_window` (Overlay layer, full anchors, keyboard None, input Full while shown), `Theme`
- Produces: `pub struct SplashGate` (pure) with `fn new(outputs: usize)`, `fn mark_ready(&mut self, output_id: u32, part: Part)` (`enum Part { Wallpaper, Bar }`), `fn ready(&self) -> bool` (true once every output has both parts), and `fn timed_out(&self, elapsed: Duration) -> bool` (true after **4s**, so a broken module never leaves the splash stuck).
  The splash draws the Sukuna wallpaper blurred and darkened (Slint has no blur, so pre-blur a 1/8-scale copy with `image::imageops::blur` once, then scale it up), the logo with a pulsing glow, and fades out over `DURATIONS.large` (from tokens) once `ready() || timed_out()`. Then the window is **destroyed**, not hidden, so its memory is released.
- [ ] **Step 1: Write failing tests:** `ready_needs_all_parts_on_all_outputs` (2 outputs, three of four parts marked gives false, all four gives true), `timeout_after_4s`, and `ignores_unknown_output`.
- [ ] **Step 2–4:** Fail, implement, pass. Hook `mark_ready` to the first `rendering_notifier` frame of each Background and Bar window.
- [ ] **Step 5: Verify:** nested start shows the splash and then a smooth fade to the shell. `tools/memcheck.sh` shows no extra RSS from the splash after it's gone, compared with 03-6's row (within 5MB). **Commit:** `git commit -m "Startup splash"`

### Task 05-2: Session menu and power button

**Files:**
- Create: `crates/caelum-services/src/session.rs`, `crates/caelum-services/tests/session.rs`, `crates/caelum-ui/ui/session/session.slint`, `crates/caelum-ui/ui/bar/power.slint`
- Modify: `crates/caelum-config/src/model.rs`, adding `SessionConfig { enabled: bool = true, drag_threshold: u32 = 30, icons: SessionIcons { logout: "logout", shutdown: "power_settings_new", hibernate: "bedtime", reboot: "cached" } }` (from `sessionconfig.hpp`)

**Interfaces:**
- Produces: `pub enum SessionAction { Logout, Shutdown, Reboot, Hibernate }`, `pub async fn perform(a: SessionAction) -> Result<(), ServiceError>`. Logout goes to `org.kde.Shutdown` `/Shutdown` `logout()`; the rest go to logind `org.freedesktop.login1.Manager` `PowerOff(false)` / `Reboot(false)` / `Hibernate(false)`. `pub async fn can(a: SessionAction) -> bool`, from logind's `CanPowerOff` etc., hides buttons the system can't do. The session drawer opens from the bar's power button and from `caelum toggle session`.
- [ ] **Step 1: Write failing tests** with a fake logind on a private bus: `reboot_calls_logind_reboot` (the fake records `Reboot(false)`) and `cannot_hibernate_hides_button` (`CanHibernate` = "na" makes `can(Hibernate)` false).
- [ ] **Step 2–4:** Fail, implement, pass.
- [ ] **Step 5: Verify:** nested. The menu opens with Caelestia's look and animation. **Only test Logout for real, inside the nested session.** Never press Shutdown or Reboot while testing. Parity screenshot. **Commit:** `git commit -m "Session menu and power button"`

### Task 05-3: Phase 1 acceptance and switch-over

- [ ] **Step 1: Memory gate:** `tools/memcheck.sh target/release/caelum-shell`, with every Phase 1 feature on 2 outputs, a tray app, and 3 notifications shown. **VmRSS ≤ 300MB.** Append the row. If it's over, stop and fix before switching.
- [ ] **Step 2: Robustness pass:** the Review Focus items from `00-overview.md`. Unplug or replug an output in the nested KWin (restart it with a different `--output-count` while Caelum runs under `tools/switch.sh` on the real desktop, or use `kscreen-doctor output.HDMI-A-1.disable` and then `.enable`). Rename the wallpaper file and restart. Write an invalid config. Flood: `for i in $(seq 500); do notify-send "n$i"; done`. Each one must behave as stated, with no crash in `journalctl --user -u caelum-shell`.
- [ ] **Step 3: Daily-drive trial:** `tools/switch.sh caelum` on the real desktop for a working day. Note every gap against Caelestia in `docs/parity/gaps.md`. Fix the gaps marked "blocker"; the rest move to file 06.
- [ ] **Step 4:** The owner decides whether to run `systemctl --user enable caelum-shell` and `systemctl --user disable caelestia-shell`. Document the reverse in the README.
- [ ] **Step 5: Commit:** `git commit -m "Phase 1 acceptance: memory, robustness, parity gaps"`
