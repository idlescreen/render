# studio

Offline saver → video rendering: the `render` engine (headless sim →
AV1/H.264/PNG/raw via ffmpeg) plus `idle-studio`, the Director TUI for
queueing and tuning export jobs. Part of
[IdleScreen](https://idlescreen.github.io) — modular Wayland screensavers
for Linux.

## Install

```sh
idlescreen install studio
```

## Commands

```sh
idlescreen studio          # the Director TUI (binary: idle-studio)
idle-studio enqueue --effect hearth --output out.mkv --duration 30s
idle-studio run --all      # drain the queue
```

```sh
render -e beams --duration 30s -o out.mkv     # AV1 in Matroska
render -e ripple --format png -o frames/      # one PNG per frame
render -e beams --stdout-raw | consumer       # raw BGRA + GBRI header
render -e beams --duration 1s --dry-run       # plan only, no encode
```

## License

Apache-2.0 · © 2026 IdleScreen
