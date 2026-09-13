# studio

Offline saver → video rendering: the `render` engine (headless sim →
AV1/H.264/PNG/raw via ffmpeg) plus `idle-studio`, the Director TUI for
queueing and tuning export jobs. Part of
[IdleScreen](https://idlescreen.github.io) — modular Wayland screensavers
for Linux.

| Path | Package | Role |
|---|---|---|
| `engine/` | `render` | render engine: `render` + `idle-render` binaries |
| `studio/` | `idle-studio` | Director TUI (drives the engine via `--job-file`) |

## Use

```sh
idlescreen studio            # or: idle-studio — the Director TUI
idle-studio enqueue --effect hearth --output out.mkv --duration 30s
idle-studio run --all        # drain the queue
```

```sh
render -e beams --duration 30s -o out.mkv            # AV1 in Matroska
render -e ripple --format png -o frames/             # one PNG per frame
render -e beams --stdout-raw | consumer              # raw BGRA + GBRI header
render -e beams --duration 1s --dry-run              # plan only, no encode
```

## Develop

Path dependency: a `runtime/` checkout inside this repo (or a symlink to a
sibling clone) provides `idle-runner`. Encoding needs `ffmpeg`.

```sh
sudo dnf install ffmpeg libdbus-1-devel wayland-devel libxkbcommon-devel \
    openssl-devel pkgconf-pkg-config                     # apt: -dev names
git clone https://github.com/idlescreen/studio.git && cd studio
git clone https://github.com/idlescreen/runtime runtime  # path dep
cargo build --workspace && cargo test --workspace
```

## License

Apache-2.0 · © 2026 IdleScreen
