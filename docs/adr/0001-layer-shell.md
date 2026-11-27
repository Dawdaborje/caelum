# ADR 0001: How Slint draws on layer-shell surfaces

Date: 2026-11-27. Status: accepted.

## Decision

**Route 3:** a custom Slint `Platform` + `WindowAdapter` on
`smithay-client-toolkit` 0.21 `LayerShell`, rendering with Slint's
`SoftwareRenderer` into SCTK `SlotPool` wl_shm buffers (`Argb8888`,
premultiplied, B,G,R,A in memory). No GL, no EGL, no GPU driver in the process.

Spike: `spike/src/main.rs`. Our own loop drives Slint:
`update_timers_and_animations()`, render dirty windows, then a calloop
dispatch that sleeps until `duration_until_next_timer_update()`.

## Routes tried

Measured with `tools/memcheck.sh` (2 outputs, 60s idle). Same bar each time:
top+left+right, 40px, exclusive zone 40, clock ticking every second,
background `primary` from the Sukuna hoodie wallpaper.

| Route | Result | VmRSS | Threads | GL contexts |
|---|---|---|---|---|
| 1. `spell-framework` 1.0.6 | Does not build against Slint 1.19 (uses internal `i_slint_core::items::MouseCursor`, removed). Its loop also wakes every 1ms per window. `layer-shika` 0.3.1 is older than 6 months. | — | — | — |
| 2. FemtoVG on EGL (glutin 0.32) | Works, **fails the 120MB gate** | 150872 kB | 14 | 2 (one per surface) |
| 3. SoftwareRenderer into wl_shm | **Passes** every gate check | 40504 kB | 1 | 0 |

Route 2's cost is the GL stack, not Slint: `libLLVM` 33MB and `libgallium`
16MB (Mesa radeonsi), plus 37MB of NVIDIA EGL libraries that glvnd loads
because the vendor ICD is installed. Pinning Mesa's EGL vendor might bring it
near 114MB, still with 2 GL contexts and ~13 driver threads per process.

Route 3's top mappings: heap 9.9MB, fontconfig caches ~14MB, the binary 6.3MB.

## Consequences

- The shell stays one thread for rendering. Every repaint is CPU work, so it
  matters to keep repaints small. The spike repaints full buffers
  (`RepaintBufferType::NewBuffer`, a fresh pool slot each frame). Phase 1
  should move to partial repaints with buffer reuse (`SwappedBuffers` on two
  buffers) before drawers and animations land, and measure CPU during
  animations.
- Large surfaces (wallpaper, full-screen overlays) cost width × height × 4
  bytes per buffer: a 1920×1080 buffer is 8MB, ×2 for double buffering, per
  output. The wallpaper should be one decoded image shared across outputs, not
  re-rendered per frame.
- Blur, shadows and heavy effects are more expensive without a GPU. If a
  Caelestia effect turns out too slow in software, revisit with a single
  shared GL context (route 2 trimmed), and measure again against the budget.
- Hotplug, input, scale factors and frame callbacks are not in the spike;
  `caelum-wayland` (task 02-4) adds them.

## Testing note

`tools/nested.sh` runs KWin with `setpriv --no-new-privs`. `kwin_wayland`
has `cap_sys_nice`, so it makes itself realtime, and a terminal with a hard
`RLIMIT_RTTIME` (rtkit gives Ghostty 200ms) gets it SIGKILLed with no log.
