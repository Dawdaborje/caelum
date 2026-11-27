# Handoff: 2026-11-27

**Switch to:** Sonnet, because Phase 0 (Opus) is done and Phase 1 tasks are well specified.
**Plan file / task / step:** 02-phase1-foundation.md / 02-1 / Step 1
**Done:** Phase 0 complete. 0-1 tools (8f36ab6, fixes 9a3ce0a), 0-2 spike + ADR (99ead18). Tracker: docs/progress.md.
**Uncommitted:** none
**Blocked on:** nothing
**Tried:** n/a
**Notes for Phase 1:**
- ADR 0001 chose route 3: custom Slint Platform on SCTK 0.21 layer-shell + SoftwareRenderer into wl_shm (40.5MB RSS, 1 thread). Start from `spike/src/main.rs`; `spike/` stays throwaway.
- Slint 1.19: `default-features = false, features = ["std", "compat-1-2", "renderer-software"]`. SCTK 0.21 needs `delegate_registry!` plus `delegate_dispatch2!`.
- Test with `just nested <bin>` / `just nested2 <bin>` / `just mem <bin>`. Nested windows pop up on the real desktop; don't close them mid-test.
- Screenshots without touching the real screen: run `spectacle -b -n -f -o file.png` inside the nested session (it uses the nested KWin's private D-Bus). See ADR testing note.
- Never `pkill`/`killall` kwin_wayland: that matches the real compositor. Kill nested ones by PID only.
**Start the next session with:**
> Read CLAUDE.md and docs/handoff.md, then continue 02-phase1-foundation.md task 02-1 step 1.
