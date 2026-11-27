# Phase 0: Layer-shell Spike and Measurement Tools

Read `00-overview.md` first (Global Constraints, Review Focus).

**Outcome:** a go/no-go decision, recorded in `docs/adr/0001-layer-shell.md`,
on how Slint draws onto layer-shell surfaces on KWin, plus the two tools every
later phase uses. The spike code itself is throwaway, in `spike/`.

---

### Task 0-1: Measurement and nested-test tools

**Files:**
- Create: `tools/nested.sh`, `tools/memcheck.sh`, `docs/memory.md`, `LICENSE` (GPL-3.0 text), `.gitignore` (`target/`)

**Interfaces:**
- Produces: `tools/nested.sh [--outputs N] <binary> [args]`, which writes `exec <binary> args` into a `mktemp` wrapper and runs `kwin_wayland --width 1600 --height 900 --output-count N --no-lockscreen --no-global-shortcuts --exit-with-session <wrapper>` (N defaults to 1). `tools/memcheck.sh <binary>` starts it through `nested.sh --outputs 2` (the budget is defined for two screens), waits 60s, then prints one markdown table row: `| date | git short sha | VmRSS kB | RssAnon kB | AnonHugePages kB | QSG/render threads |`. It also prints the top 10 mappings by RSS (same awk as the Caelestia investigation) to stderr. Both kill the nested KWin on exit.

- [ ] **Step 1:** Write both scripts with `set -euo pipefail` and a `trap` that cleans up. `memcheck.sh` finds the child PID with `pgrep -n -f "<binary>"`.
- [ ] **Step 2: Verify:** `tools/memcheck.sh /usr/bin/weston-simple-shm || tools/memcheck.sh foot` prints a row with a non-empty VmRSS, and no `kwin_wayland --width` process is left afterwards (`pgrep -f 'kwin_wayland --width'` exits 1).
- [ ] **Step 3:** Create `docs/memory.md` with the table header and a first row recording Caelestia's measured baseline: `| 2026-11-27 | caelestia c90c40f | 780040 | ~460000 | 0 | 13 |`.
- [ ] **Step 4: Commit:** `git commit -m "Add nested-KWin and memory measurement tools"`

### Task 0-2: Spike: Slint bar on a layer-shell surface

**Files:**
- Create: `spike/Cargo.toml`, `spike/src/main.rs`, `spike/ui/bar.slint`

Try these routes **in order**, stopping at the first one that passes the gate
below:

1. A maintained crates.io crate that gives Slint layer-shell windows on Wayland.
   Search crates.io and GitHub for "slint layer shell". Check that it was
   released in the last 6 months and supports a current Slint.
2. A custom Slint platform (`slint::platform::Platform` + `WindowAdapter`)
   using `smithay-client-toolkit`'s `LayerShell`, with Slint's FemtoVG
   renderer on an EGL context created through `wayland-egl` + `glutin`.
3. Same as 2 but with Slint's `SoftwareRenderer` writing into SCTK `SlotPool`
   shm buffers. This is likely cheapest in memory for small surfaces.

The bar: anchored top+left+right, height 40, `exclusive_zone` 40, showing a
`Text` with the current time (updated every second by `slint::Timer`) on a
background coloured with `primary` from the Sukuna hoodie wallpaper (use the
`material-colors` crate directly here).

- [ ] **Step 1:** Build route 1 (or 2/3) and run `tools/nested.sh spike/target/release/caelum-spike`.
- [ ] **Step 2: Gate:** all of the following must hold:
  - the bar draws at the top of the nested KWin window, and a `konsole` started inside the nested session maximises *below* it
  - the clock ticks
  - the bar draws on both outputs under `tools/nested.sh --outputs 2`
  - `tools/memcheck.sh spike/target/release/caelum-spike` shows VmRSS **≤120MB**
- [ ] **Step 3:** If a route fails the gate, record why and try the next one. If all three fail, stop and bring the measurements to the owner before going any further.
- [ ] **Step 4:** Write `docs/adr/0001-layer-shell.md`: the route chosen, the RSS of each route tried, the renderer, and how many GL contexts or threads it uses. Append the chosen route's row to `docs/memory.md`.
- [ ] **Step 5: Commit:** `git commit -m "Spike: Slint on layer-shell; record decision"`
