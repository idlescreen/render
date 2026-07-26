# render

Offline video rendering for IdleScreen: run saver plugin math and encode frames (AV1 via ffmpeg when available).

Website: [https://idlescreen.github.io](https://idlescreen.github.io)

Used by **idle-studio** for queued jobs; can also be invoked directly.

### CLI examples

```bash
render --effect beams --duration 10s -o /tmp/beams.mkv
render --effect ripple --duration 10s --width 1280 --height 720 -o /tmp/ripple.mkv
render --effect storm --duration 8h --segment 1h --width 3840 --height 2160 \
  --preset 10 --resume -o /tmp/night.mkv
```

Notes:

- Size is `--width` / `--height` (no `--resolution` preset).
- `--segment` enables long segmented encodes (e.g. `1h`); parts are `out.part000.mkv`….
- `--resume` skips encode for existing non-empty parts (still advances the sim).
- `--crf` (default 35) and `--preset` control quality/speed (SVT numeric; NVENC `p1`–`p7`).
- **GPU-aware by default:**
  - Cell raster may use **wgpu** (via idle-runner).
  - **Hardware AV1** preferred when it actually works (`av1_nvenc` / `qsv` / `amf` are
    probed; broken drivers fall back to `libsvtav1` / aom / rav1e).
  - `--no-gpu-upscale` asks idle-runner for CPU upscale; `--no-hw-encode` skips HW auto-detect.
  - `--encoder NAME` forces a codec (no auto-fallback).
