# Studio readiness (host + render)

Goal: **IdleScreen host** and **render** are solid enough that Studio can focus on
UI/workflow instead of fighting missing plugins, broken CLI defaults, or flaky
daemon installs.

## IdleScreen host (desktop product)

| Check | How | Status target |
|-------|-----|----------------|
| Install | `curl …/install.sh \| sh` or `dnf install idlescreen` | full stack + meta **≥ 2.6.1** |
| Daemon | `systemctl --user is-active idle-daemon` | active, NRestarts=0 |
| Doctor | `idlescreen doctor` | NOMINAL when not media-inhibited |
| Preview | `idlescreen preview beams` then `stop` | overlay works |
| Savers | `idlescreen saver list` + 10 `.so` under `/usr/libexec/idle/screensavers` | present |
| Remove | `sudo dnf remove idlescreen` | stack + repo gone; no orphan daemon |
| Caption font | journal: `loaded caption font from …dejavu…` | no warn |
| Auth spam | journal on control calls | no `/proc/…/exe` warn |

**Out of scope for “host good”:** TUI cosmetics, COSMIC panel dock state, Studio UI.

**Known non-blockers:** `gpu_enabled` always false (deprecated); media inhibitors
(Firefox) correctly block *idle* savers but not forced preview.

## Render capability

| Check | How | Status target |
|-------|-----|----------------|
| Unit tests | `cargo test -p render` | green |
| Dry-run | `render -e beams --duration 1s --dry-run` | prints plan, no encode |
| Encode smoke | `render -e beams --duration 1s -o /tmp/t.mkv` | Matroska file written |
| Plugin discovery | uses `/usr/libexec/idle/screensavers` when effect name given | no path required |
| Job file | `render --job-file job.json` | Studio contract |
| Dual bins | `default-run = "render"` | `cargo run -p render` works |
| ffmpeg | `ffmpeg -version` | available for AV1 |

**Render ≠ host GPU flag:** offline `gpu_upscale` is a separate export path; host
`gpu_enabled` remains deprecated.

## Studio (TUI-first)

| Check | How |
|-------|-----|
| Build | `cargo build --release -p idle-studio` |
| Open TUI | `idle-studio` (no args) or `idle-studio tui` |
| New job | In TUI: `n` → set params → `s` |
| Run | In TUI: `Enter` / `r` / `a` (needs `render` on PATH) |
| Scripting | optional `enqueue` / `list` / `run` subcommands |

Host + render rows above should be green before relying on Studio day-to-day.

## Housekeeping backlog (host / render only)

1. ~~Dual-bin default-run (render, studio)~~
2. ~~Dry-run without required `-o`~~
3. Debian changelog lag on host packages (keep in sync with RPM cuts)
4. Optional: coalesce ScreenSaver inhibit cookies when many Firefox media holds
5. Optional: ship `render` / `idle-studio` on the idlescreen package channel
6. Package-time gate for render (mirror host `qa_package_gate`)

Update this table when cuts land.
