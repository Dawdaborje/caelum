# Caelum

A Rust + [Slint](https://slint.dev) rewrite of the Caelestia KDE shell: same design and behaviour, much less memory (target ≤ 300MB idle RSS, vs ~780MB for Caelestia on Quickshell). Runs on KWin in a Plasma 6 Wayland session. Also covers a Sukuna-themed boot animation, login screen and startup splash.

## Inspiration

Caelum is inspired by and reimplements the design of [Caelestia](https://github.com/caelestia-kde/caelestia) ([my fork](https://github.com/Dawdaborje/caelestia)). All credit for the original look and behaviour goes to the Caelestia contributors.

**Status:** Phase 1 in progress (wallpaper and border frame so far).

## Build

```sh
cargo build --release -p caelum-shell
```

Config lives at `~/.config/caelum/config.toml`; see [`config.example.toml`](config.example.toml).

## Test in a nested KWin

Runs in its own KWin window without touching the real desktop:

```sh
tools/nested.sh --outputs 2 target/release/caelum-shell
```

## Switch the real desktop

```sh
tools/switch.sh caelum     # build, install to ~/.local/bin + user unit, start; stops Caelestia
tools/switch.sh caelestia  # rollback: starts Caelestia, which stops Caelum
```

The two units conflict, so only one runs at a time. The script never enables
Caelum; enable it yourself (`systemctl --user enable caelum-shell`) once you
want it at login. If the desktop is unusable, switch to a TTY and run
`systemctl --user start caelestia-shell`.

- Design: [`docs/superpowers/specs/2026-11-27-caelum-design.md`](docs/superpowers/specs/2026-11-27-caelum-design.md)
- Plan: [`docs/superpowers/plans/2026-11-27-caelum/`](docs/superpowers/plans/2026-11-27-caelum/) (start with `00-overview.md`)

## License

GPL-3.0-or-later, same as Caelestia. See [`LICENSE`](LICENSE).
