# IdleScreen render

**Offline export capability** for IdleScreen (library + `render` CLI).

Studio (UI) lives in [`studio/`](studio/) in **this same repo** and drives render via job files.

```text
studio (UI)  →  render (capability)  →  video
                     │
                     includes draw, upscale, encode
```

### Layout

```text
render/                 # this repo
  engine/               # capability (library + render binary)
  studio/               # UI (idle-studio binary)
```

### Build

```bash
# needs sibling idlescreen/idle checkout (path ../../idle from engine/)
cargo build --release -p render -p idle-studio
```

### CLI

```bash
render -e ripple --duration 10s -o /tmp/ripple.mkv
render -e ripple --duration 10s --width 3840 --height 2160 --fps 30 -o /tmp/4k.mkv
render --job-file /path/to/job.json
```

### Job file (Studio contract)

```json
{
  "effect": "ripple",
  "duration": "10s",
  "output": "/tmp/ripple.mkv",
  "width": 1280,
  "height": 720,
  "fps": 30,
  "prefer_hw": true,
  "gpu_upscale": true
}
```

```bash
render --job-file job.json
```

### Notes

- **Resolution:** `--width` / `--height`
- **Framerate:** `--fps`
- **Long jobs:** `--segment 1h` + `--resume`
- **HW encode:** preferred when probe succeeds; `--no-hw-encode` for software only
- **Upscale** is an internal frame step (not a separate product)

Website: [https://idlescreen.github.io](https://idlescreen.github.io)
