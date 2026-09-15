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
idlescreen studio                             # Director TUI: queue + tune jobs
render -e beams --duration 30s -o out.mkv     # headless render → AV1/Matroska
render -e ripple --format png -o frames/      # one PNG per frame
```

## License

Apache-2.0 · © 2026 IdleScreen

---

<div align="center">

[![Necrometer](necrometer.svg)](https://necrometer.dev/?u=idlescreen)

</div>
