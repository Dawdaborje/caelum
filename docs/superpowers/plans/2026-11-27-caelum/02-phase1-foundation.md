# Phase 1a: Foundation

Read `00-overview.md` and `docs/adr/0001-layer-shell.md` first. Everything
surface-related here uses the route that the ADR chose.

**Outcome:** a `caelum-shell` binary that starts on nested KWin, draws the
wallpaper on every output with a themed border frame, reloads its config live,
and installs as a user unit.

---

### Task 02-1: Workspace skeleton

**Files:**
- Create: `Cargo.toml` (workspace, `resolver = "2"`, members `crates/*`), `crates/{caelum-shell,caelum-ui,caelum-wayland,caelum-services,caelum-theme,caelum-config}/Cargo.toml` + `src/lib.rs` (`main.rs` for shell), `rust-toolchain.toml` (`channel = "stable"`), `.github/workflows/ci.yml` (fmt, clippy `-D warnings`, test)

- [x] **Step 1:** Create the crates. `caelum-ui` has a `build.rs` that calls `slint_build::compile("ui/main.slint")`, with a stub `export component Root inherits Window {}` in `ui/main.slint`.
- [x] **Step 2: Verify:** `cargo build --workspace && cargo clippy --workspace -- -D warnings && cargo fmt --check` all exit 0.
- [x] **Step 3: Commit:** `git commit -m "Workspace skeleton"`

### Task 02-2: Config (`caelum-config`)

**Files:**
- Create: `crates/caelum-config/src/{lib.rs,model.rs,watch.rs}`, `crates/caelum-config/tests/config.rs`, `config.example.toml`

**Interfaces:**
- Produces:
  - `pub struct Config { pub wallpaper: WallpaperConfig, pub border: BorderConfig, pub bar: BarConfig, pub notifs: NotifsConfig, pub launcher: LauncherConfig, pub osd: OsdConfig }`. Every struct derives `Deserialize, Clone, Debug, PartialEq`, has `#[serde(default)]`, and implements `Default` with Caelestia's defaults.
  - `WallpaperConfig { path: PathBuf, rotation: Vec<PathBuf>, dark: bool }`. Default `path` = `$HOME/Pictures/wallpaper/anime/sukuna-hoodie-3840x2160-26053.jpg`, `rotation` = the 3 Sukuna paths from Global Constraints, `dark` = true.
  - `BorderConfig { thickness: u32 /*10*/, rounding: u32 /*25*/, smoothing: u32 /*20*/ }`
  - `BarConfig { position: Edge /*copy default from barconfig.hpp*/, ... }`, `NotifsConfig`, `LauncherConfig`, `OsdConfig`: empty structs for now. Tasks 03-1, 04-2, 04-3, 04-4 and 05-2 add their fields (with the defaults those tasks list). Later phases add more the same way.
  - `pub enum Edge { Top, Bottom, Left, Right }` (serde lowercase)
  - `pub fn config_path() -> PathBuf`, which resolves `$XDG_CONFIG_HOME/caelum/config.toml`
  - `pub fn load(path: &Path) -> Result<Config, ConfigError>`. A missing file returns `Ok(Config::default())`.
  - `pub fn watch(path: PathBuf, on_change: impl Fn(Config) + Send + 'static) -> Result<WatchHandle, ConfigError>`, using the `notify` crate with a 200ms debounce. On a parse error it logs `warn!` and does **not** call `on_change`.

- [x] **Step 1: Write failing tests** in `tests/config.rs`:
  - `missing_file_is_default`: `load("/nonexistent")` == `Ok(Config::default())`
  - `defaults_match_caelestia`: `Config::default().border` == `BorderConfig { thickness: 10, rounding: 25, smoothing: 20 }`
  - `partial_file_fills_defaults`: `[border]\nthickness = 4` yields thickness 4, rounding 25
  - `unknown_edge_is_error`: `[bar]\nposition = "middle"` returns `Err`
  - `invalid_reload_keeps_previous`: watch a temp file. Write valid config A, then write `[[[`, then wait 500ms. Exactly one callback has fired, and it carried A.
- [x] **Step 2:** `cargo test -p caelum-config` fails (types missing).
- [x] **Step 3:** Implement. Write `config.example.toml` with every field at its default and a comment on each.
- [x] **Step 4:** `cargo test -p caelum-config` passes.
- [x] **Step 5: Commit:** `git commit -m "Config loading and live reload"`

### Task 02-3: Theme (`caelum-theme`)

**Files:**
- Create: `crates/caelum-theme/src/{lib.rs,scheme.rs,tokens.rs}`, `crates/caelum-theme/tests/theme.rs`, `crates/caelum-theme/tests/fixtures/red.png` (16×16 solid #c0392b, generated in the test's build step or committed; it is not a wallpaper)

**Interfaces:**
- Produces:
  - `pub struct Rgba(pub u8, pub u8, pub u8, pub u8)`
  - `pub struct Scheme { pub primary, on_primary, primary_container, on_primary_container, secondary, on_secondary, secondary_container, on_secondary_container, tertiary, on_tertiary, error, on_error, surface, on_surface, surface_variant, on_surface_variant, surface_container_lowest, surface_container_low, surface_container, surface_container_high, surface_container_highest, outline, outline_variant, shadow, inverse_surface, inverse_on_surface, inverse_primary: Rgba }`. These are the same role names Caelestia's `src/matugen/scheme.json.tmpl` uses: check that file and add any roles it has that this list misses.
  - `pub fn scheme_from_image(path: &Path, dark: bool) -> Result<Scheme, ThemeError>`. It decodes with `image` and **downscales to 128×128 before quantising**: Caelestia's analyser lesson, never decode at full 4K into the shell's long-lived memory.
  - `pub fn Scheme::fallback(dark: bool) -> Scheme`, seeded from `#6750A4`.
  - `pub fn scheme_or_fallback(path: &Path, dark: bool) -> Scheme`, which logs `warn!` on error.
  - `pub mod tokens`: sizes come from `Config`, so this module holds only the **animation curves and durations** copied from `~/caelestia/shell/plugin/src/Caelestia/Config/anim.cpp`, as `pub struct Curve { pub x1: f32, pub y1: f32, pub x2: f32, pub y2: f32 }` constants named after Caelestia's (`EMPHASIZED`, `EMPHASIZED_ACCEL`, `EMPHASIZED_DECEL`, `STANDARD`, `STANDARD_ACCEL`, `STANDARD_DECEL`, `EXPRESSIVE_FAST_SPATIAL`, `EXPRESSIVE_DEFAULT_SPATIAL`, `EXPRESSIVE_SLOW_SPATIAL`, `EXPRESSIVE_FAST_EFFECTS`, `EXPRESSIVE_DEFAULT_EFFECTS`, `EXPRESSIVE_SLOW_EFFECTS`) and `pub const DURATIONS: Durations { small, normal, large, extra_large, expressive_fast_spatial, expressive_default_spatial, expressive_slow_spatial, expressive_fast_effects, expressive_default_effects, expressive_slow_effects: u32 /*ms*/ }`, the same fields as `AnimDurations` in `Config/appearanceconfig.hpp` (its getters in the matching `.cpp` hold the values). If Caelestia's curve is a multi-segment `QEasingCurve::BezierSpline`, keep every segment: `pub struct Curve(pub &'static [[f32; 6]])`.

- [x] **Step 1: Write failing tests:**
  - `red_image_gives_reddish_primary`: hue of `scheme_from_image(red.png, true).primary` is within 30° of 6° (#c0392b's hue)
  - `missing_wallpaper_uses_fallback`: `scheme_or_fallback("/nope.jpg", true)` == `Scheme::fallback(true)`
  - `dark_and_light_differ`: `surface` luminance is lower in dark than in light for the same image
  - `curves_match_caelestia`: `EMPHASIZED_DECEL` equals the control points in `anim.cpp` (the test hard-codes the values the implementer copied, with a comment citing the line)
- [x] **Step 2:** `cargo test -p caelum-theme` fails.
- [x] **Step 3:** Implement with the `material-colors` crate (`theme_from_source_color`, picking the source with its `score`/quantizer).
- [x] **Step 4:** Tests pass. Also run `cargo run -p caelum-theme --example dump -- ~/Pictures/wallpaper/anime/sukuna-hoodie-3840x2160-26053.jpg` (write this 10-line example), which prints the scheme as hex. Paste the output into `docs/sukuna-scheme.md`: phases 4 and 5 use those values.
- [x] **Step 5: Commit:** `git commit -m "Material You scheme from wallpaper; Caelestia animation tokens"`

### Task 02-4: Surfaces and outputs (`caelum-wayland`)

**Files:**
- Create: `crates/caelum-wayland/src/{lib.rs,outputs.rs,surface.rs,protocols.rs}`, `crates/caelum-wayland/protocols/*.xml` (copied from Caelestia), `crates/caelum-wayland/tests/outputs.rs`

**Interfaces:**
- Consumes: the route from ADR 0001.
- Produces:
  - `pub struct OutputInfo { pub id: u32, pub name: String, pub width: i32, pub height: i32, pub scale: f64 }`
  - `pub enum OutputEvent { Added(OutputInfo), Removed(u32), Changed(OutputInfo) }`
  - `pub struct LayerSpec { pub namespace: &'static str, pub layer: Layer, pub anchors: Anchors, pub size: (u32, u32), pub exclusive_zone: i32, pub keyboard: KeyboardMode, pub input_region: InputRegion }` with `Layer { Background, Bottom, Top, Overlay }`, `KeyboardMode { None, OnDemand, Exclusive }` and `InputRegion { Full, Empty, Rects(Vec<(i32,i32,i32,i32)>) }`
  - `pub fn create_layer_window(output_id: u32, spec: LayerSpec) -> Result<slint::Window-backed handle, WaylandError>`. The exact return type is whatever the ADR route gives; name it `LayerWindow` and give it `fn set_exclusive_zone(&self, i32)`, `fn set_input_region(&self, InputRegion)`, `fn set_size(&self, u32, u32)`, `fn slint_window(&self) -> &slint::Window`.
  - `pub struct OutputTracker`, a pure state machine fed by Wayland events, with `fn apply(&mut self, ev: OutputEvent) -> Vec<SurfaceAction>` where `enum SurfaceAction { Create(u32), Destroy(u32), Resize(u32) }`. It is kept pure so it can be tested without a compositor.
- [x] **Step 1: Write failing tests** for `OutputTracker` only:
  - `added_creates_once`: Added(1) twice gives `[Create(1)]` then `[]`
  - `outputs_removed_drops_surfaces`: Added(1), Added(2), Removed(1) ends with `[Destroy(1)]`
  - `changed_resizes`: Added(1), then Changed(1 with new width), gives `[Resize(1)]`
- [x] **Step 2:** Tests fail. **Step 3:** Implement `OutputTracker`, then the Wayland side from the spike's code (refactored, no longer throwaway).
- [x] **Step 4:** Tests pass. **Manual check:** under `tools/nested.sh --outputs 2`, a debug example creates one 1×1 transparent `Top` surface per output and logs Added/Removed.
- [x] **Step 5: Commit:** `git commit -m "Layer surfaces and output tracking"`

### Task 02-5: Shell window, wallpaper and border frame

**Files:**
- Create: `crates/caelum-shell/src/{main.rs,app.rs,theme_bridge.rs}`, `crates/caelum-ui/ui/{main.slint,theme.slint,background.slint,border.slint}`

**Interfaces:**
- Consumes: `Config`, `scheme_or_fallback`, `create_layer_window`, `OutputTracker`
- Produces:
  - `theme.slint`: `export global Theme { in property <color> primary; …every Scheme role…; in property <length> border-thickness; in property <length> border-rounding; }`
  - `theme_bridge::apply(ui: &impl ComponentHandle, scheme: &Scheme, cfg: &Config)`, which sets every `Theme` property
  - Per output: one `Background` layer window (layer `Background`, full anchors, exclusive −1, input Empty) drawing the wallpaper scaled to the output size with `Image::load_from_path` **decoded at output size** (resize with `image` to the output's pixel size before handing to Slint). Plus one `Border` window (layer `Top`, full anchors, input Empty) drawing Caelestia's rounded frame: a filled `Theme.surface` frame `border-thickness` wide with inner corners rounded `border-rounding`.
  - Plus the exclusion surfaces: **one** per output edge that has a reserved zone (in Phase 1 only the bar's edge plus the border thickness on the others), not four always-on windows.
- [ ] **Step 1:** Implement. `main.rs` initialises `tracing_subscriber`, loads the config, computes the scheme, creates windows per `SurfaceAction`, and hooks `watch()` to re-apply the theme (and recompute the scheme if `wallpaper.path` changed) via `slint::invoke_from_event_loop`.
- [ ] **Step 2: Verify visually:** `tools/nested.sh --outputs 2 target/release/caelum-shell` shows the Sukuna wallpaper and a rounded frame on both outputs. Put a side-by-side screenshot with Caelestia (Spectacle on the real desktop vs the nested window) in `docs/parity/02-5-border.png`. The frame width and corner radius should match by eye at 100% zoom.
- [ ] **Step 3: Verify reload:** with it running, change `border.thickness` to 20 in the config. The frame updates within 1s with no restart.
- [ ] **Step 4: Measure:** `tools/memcheck.sh target/release/caelum-shell` and append the row. Expected: well under 120MB.
- [ ] **Step 5: Commit:** `git commit -m "Wallpaper and border frame per output"`

### Task 02-6: User unit and switch scripts

**Files:**
- Create: `dist/caelum-shell.service`, `tools/switch.sh`, `README.md` (build, nested test, switch, rollback)

**Interfaces:**
- Produces: `dist/caelum-shell.service`:
  ```ini
  [Unit]
  Description=Caelum Shell
  PartOf=graphical-session.target
  After=graphical-session.target
  Before=xdg-desktop-autostart.target
  Conflicts=caelestia-shell.service

  [Service]
  Type=exec
  ExecStart=%h/.local/bin/caelum-shell
  Restart=on-failure
  RestartSec=5s
  TimeoutStopSec=5s
  Slice=session.slice
  MemoryTHP=madvise
  ```
  `tools/switch.sh caelum|caelestia`. `caelum` runs `cargo build --release`, installs the binary and unit to `~/.local/bin` and `~/.config/systemd/user`, `daemon-reload`, `start caelum-shell`. `caelestia` runs `systemctl --user start caelestia-shell`, which stops Caelum through `Conflicts=` in reverse. The script does **not** `enable` Caelum: the owner enables it manually once Phase 1 is complete.
- [ ] **Step 1:** Write both files and the README.
- [ ] **Step 2: Verify:** `systemd-analyze --user verify dist/caelum-shell.service` prints nothing. Then `tools/switch.sh caelum`: Caelestia stops and Caelum draws on the real desktop. Then `tools/switch.sh caelestia`: Caelestia returns and `systemctl --user is-active caelum-shell` prints `inactive`.
- [ ] **Step 3: Commit:** `git commit -m "User unit and switch script"`
