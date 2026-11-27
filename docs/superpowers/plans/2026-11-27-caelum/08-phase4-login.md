# Phase 4: Login Screen (option L1: restyled SDDM theme)

Read `00-overview.md` first. This phase can start any time after task 02-3,
because it needs `docs/sukuna-scheme.md`.

**Reference:** `~/caelestia/src/sddm/themes/full/` (QML theme, Theme-API 2.0,
Qt 6) and `~/caelestia/src/sddm/sync.sh` (how it copies the wallpaper and
colours in as root).

**Key facts:**
- SDDM reads `/etc/sddm.conf.d/*.conf` alphabetically, and **the last value
  wins**. This machine currently has several files setting `Current=`
  (material-you, panacea, sddm-astronaut-theme, caelestia). Caelum adds
  `zz-caelum.conf`, which overrides them without deleting anything.
  Rollback is deleting that one file.
- The theme is named `caelum` and installed to `/usr/share/sddm/themes/caelum`,
  so Caelestia's own `sync.sh` never overwrites it.
- The wallpaper is copied in at **install time** from the owner's Pictures
  folder, never committed.

---

### Task 08-1: Caelum SDDM theme

**Files:**
- Create: `login/caelum/` (copied from `~/caelestia/src/sddm/themes/full/`, keeping its GPL headers), `login/caelum/metadata.desktop` (`Name=Caelum`, `Description=Caelum login, Sukuna theme`), `login/caelum/theme.conf` (colours from `docs/sukuna-scheme.md`), `login/install.sh`, `login/README.md`, `login/tests/check.sh`

**Interfaces:**
- Produces: `login/install.sh`, run with `sudo`. It installs `login/caelum/` to `/usr/share/sddm/themes/caelum/`, copies `$REAL_USER`'s `~/Pictures/wallpaper/anime/sukuna-hoodie-3840x2160-26053.jpg` downscaled to 1920px wide as `background.jpg` (using `magick`, which follows `sync.sh`'s size-capped copy pattern), and writes `/etc/sddm.conf.d/zz-caelum.conf` containing `[Theme]\nCurrent=caelum`. `login/install.sh --uninstall` removes both.
- [ ] **Step 1:** Copy the theme. Replace every colour in `theme.conf` with the role values from `docs/sukuna-scheme.md` (keep the key names), and swap `assets/background.png` for a reference to `background.jpg`. Remove the `dino.png` easter egg only if the owner says so (keep it by default).
- [ ] **Step 2: Test in test mode, no install:** `sddm-greeter-qt6 --test-mode --theme login/caelum`. The greeter opens in a window, colours match the shell's, the clock works, and the session picker lists Plasma. Parity screenshot `docs/parity/08-1-login.png`.
- [ ] **Step 3:** `login/tests/check.sh` runs `qmllint` on every QML file (the same rules as Caelestia's `.github/scripts/qmllint`) and checks that `theme.conf` has no colour left from the old palette (a grep for the old hex values from Caelestia's `theme.conf` gives 0 hits). Run it and expect exit 0.
- [ ] **Step 4: Install and test for real:** `sudo login/install.sh`, then log out. The Caelum greeter shows, logging in works, and a wrong password shows the error state. **Rollback check:** `sudo login/install.sh --uninstall`, log out, and the previous greeter is back. Re-install afterwards.
- [ ] **Step 5: Commit:** `git commit -m "Caelum SDDM theme with Sukuna palette"`
