# Phase 5: Boot Animation (Plymouth, optional GRUB theme)

Read `00-overview.md` first. It can start any time after task 02-3
(`docs/sukuna-scheme.md`).

**Machine facts (checked 2026-11-27):** Plymouth isn't installed. The bootloader
is GRUB (`/boot/grub/grub.cfg`). The kernel command line is `… loglevel=3
quiet`, with no `splash`. The root filesystem is btrfs and **not encrypted**, so no
password prompt is needed. Initramfs comes from mkinitcpio (Arch default:
check `/etc/mkinitcpio.conf` for `HOOKS` and record it before changing it).

**Safety rule:** every step that touches `/etc` or `/boot` first saves the
original to `boot/backup/<file>.orig`, and the rollback in
`boot/README.md` is **tested before the phase is done**. Keep the GRUB menu
reachable (don't set `GRUB_TIMEOUT=0` or hidden) so a bad initramfs can
always be bypassed by editing the kernel line.

---

### Task 09-1: Frame generator

**Files:**
- Create: `boot/framegen/` (a small binary crate in the workspace), `boot/framegen/tests/frames.rs`

**Interfaces:**
- Consumes: `caelum-theme::scheme_from_image`
- Produces: `caelum-framegen --wallpaper <path> --out <dir> --frames 60 --size 1920x1080`, which writes:
  - `background.png`: the wallpaper scaled to fill `--size`, darkened 40% and blurred, so the animation reads on top of it
  - `logo.png`: `crates/caelum-ui/assets/caelum-logo.svg` rendered at 256px with `resvg`. If 05-1 hasn't run yet, create the logo here as 05-1 describes it.
  - `throbber-00.png` … `throbber-59.png`: a 96px ring arc spinning one turn over the 60 frames, drawn in `primary` from the scheme with a `tertiary` glow
- [ ] **Step 1: Write failing tests:** `writes_expected_files` (60 throbber frames plus 2 more in a temp dir), `background_matches_size` (1920×1080), `throbber_uses_primary` (the brightest pixel of frame 0 is within ΔE 10 of `scheme.primary`).
- [ ] **Step 2–4:** Fail, implement (`image`, `tiny-skia`, `resvg`), pass.
- [ ] **Step 5: Commit:** `git commit -m "Plymouth frame generator"`

### Task 09-2: Plymouth theme

**Files:**
- Create: `boot/plymouth/caelum/caelum.plymouth`, `boot/plymouth/caelum/caelum.script`, `boot/install.sh`, `boot/README.md`

**Interfaces:**
- Produces:
  - `caelum.plymouth`: `[Plymouth Theme]` `Name=Caelum`, `ModuleName=script`, `[script]` `ImageDir=/usr/share/plymouth/themes/caelum`, `ScriptFile=/usr/share/plymouth/themes/caelum/caelum.script`
  - `caelum.script`: draws `background.png` scaled to the screen, `logo.png` centred, and the throbber frames cycling at 30fps below the logo via `Plymouth.SetRefreshFunction`. On `Plymouth.SetQuitFunction` it fades everything out over 0.3s. Messages (`Plymouth.SetMessageFunction`) appear as small text under the throbber in `on_surface`.
  - `boot/install.sh` (sudo):
    1. `pacman -S --needed plymouth`
    2. back up `/etc/mkinitcpio.conf` and `/etc/default/grub`
    3. run `caelum-framegen` with the Sukuna hoodie wallpaper into `/usr/share/plymouth/themes/caelum/`, then copy the `.plymouth` and `.script`
    4. add `plymouth` to `HOOKS` in `mkinitcpio.conf` right after `udev` (or after `systemd` if the hooks are systemd-based)
    5. add `splash` to `GRUB_CMDLINE_LINUX_DEFAULT` (keep `quiet loglevel=3`)
    6. `plymouth-set-default-theme caelum`, `mkinitcpio -P`, `grub-mkconfig -o /boot/grub/grub.cfg`
  - `boot/install.sh --uninstall`: restores both backups, runs `plymouth-set-default-theme -R bgrt`, `mkinitcpio -P` and `grub-mkconfig`. Plymouth stays installed but unused.
- [ ] **Step 1: Test without rebooting first:** `sudo plymouthd --debug --tty=/dev/tty2; sudo plymouth --show-splash; sleep 8; sudo plymouth --quit` from a spare TTY (Ctrl+Alt+F2, then come back with F1). It shows the background, logo and spinning throbber. Fix the script until it looks right.
- [ ] **Step 2:** `sudo boot/install.sh`, then reboot. The Caelum animation shows from early boot until SDDM.
- [ ] **Step 3: Rollback check:** `sudo boot/install.sh --uninstall`, reboot, and it boots normally with the old text boot. Then reinstall.
- [ ] **Step 4:** Write `boot/README.md` with: what was changed, the rollback command, and the emergency fix (at the GRUB menu press `e`, delete `splash` from the `linux` line, and boot with Ctrl+X).
- [ ] **Step 5: Commit:** `git commit -m "Caelum Plymouth boot animation"`

### Task 09-3 (optional, owner decides): GRUB menu theme

Do this only if the owner asks for it after 09-2.

**Files:**
- Create: `boot/grub/caelum/theme.txt`, plus assets from `caelum-framegen --grub` (background at 1920×1080, `select_*.png` highlight slices in `primary_container`)
- [ ] **Step 1:** Write `theme.txt` (a boot menu centred over the background, `item_color` = `on_surface`, `selected_item_color` = `on_primary_container`, `terminal-font` from `grub-mkfont` of the shell's UI font at 16px).
- [ ] **Step 2:** Install to `/boot/grub/themes/caelum/`, set `GRUB_THEME=` in `/etc/default/grub` (after backing it up), and `grub-mkconfig`. Reboot to see it. The rollback is removing `GRUB_THEME` and regenerating.
- [ ] **Step 3: Commit:** `git commit -m "Optional GRUB theme"`
