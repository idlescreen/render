# render

Offline export capability for [IdleScreen](https://github.com/idlescreen/idle)
— runs a saver plugin's simulation headless and encodes the frames to video
(AV1 default, H.264 for `--container mp4`), PNG sequences, or raw BGRA.
[`idle-studio`](https://github.com/idlescreen/idle-studio) is the TUI that
drives this via `--job-file`; `render` is the engine underneath it.

## Usage

```sh
render -e beams --duration 30s -o out.mkv          # AV1 in Matroska
render -e hearth --duration 5m --segment 1m -o h.mkv   # segmented + concat
render -e ripple --format png -o frames/           # one PNG per frame
render -e beams --stdout-raw | consumer            # raw BGRA + GBRI header
render --job-file job.json                         # JSON contract (Studio)
render -e beams --duration 1s --dry-run            # plan only, no encode
```

`idle-render` is the same binary under a namespaced name — installed
alongside `render` so `/usr/bin/render` collisions aren't fatal.

Notable flags: `--seed`, `--fps`, `--width/--height` (or `--cols/--rows` for
grid), `--crf` (0–63), `--preset`, `--encoder` (force an ffmpeg encoder;
auto-detection probes hardware first then falls back to libsvtav1/libx264),
`--no-hw-encode`, `--cpu-raster` (deterministic, bypasses GPU variance),
`--audio` (loop/shorten a bed to video length), `--resume` (skip existing
segment parts, still advances the sim), `--baseline-dir` +
`--snapshot-last-only` (byte-compare the last frame for determinism CI —
`--update-baselines` is gated behind `RENDER_FORCE_UPDATE_BASELINES=1`).

Plugin resolution: `--plugin-path foo.so` loads an explicit saver library
(unsigned loads need `IDLE_ALLOW_UNSIGNED_PLUGINS=1`); otherwise the effect
name resolves through idle-runner's signed-manifest discovery.

## Exit codes

| Code | Meaning |
|---|---|
| 0 | success |
| 1 | pipeline/encode error |
| 2 | argument-parse error |
| 3 | snapshot baseline missing |
| 4 | snapshot mismatch |

## Layout

`engine/` is the workspace member (lib `idle_render` + `render`/`idle-render`
bins). The build needs a sibling `idle` checkout — `./idle` is a symlink or
real checkout of `idlescreen/idle` (provides `idle-runner`, `idle-api`, the
wayland/dbus crates). CI creates it automatically; locally,
`scripts/package.sh` symlinks `../idle` when present.

Packaging: signed RPM/DEB via release workflow; `scripts/package.sh` runs the
local gate (`qa_package_gate.sh`) then builds both into the packages pool.
The `Dockerfile` + `unraid/render.xml` are placeholders — no container image
is published yet.
