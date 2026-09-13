# idle-studio

Director for offline IdleScreen renders — queue export jobs, tune
parameters, and run them through the `render` engine (saver math → AV1
via ffmpeg). TUI-first; no display server required.

## Using it

```sh
idle-studio            # opens the Director TUI (the product)
idle-studio tui        # same thing, explicit
idle-studio enqueue --effect hearth --output out.mkv --duration 30s
idle-studio list
idle-studio run        # next pending job
idle-studio run --all  # drain the queue
```

Queue state lives at `~/.config/idle-studio/queue.json` — writes are
atomic (tmp + fsync + rename, mode 0600), a corrupt queue is moved aside
to `queue.json.corrupt-<ts>` rather than crashing the TUI, and jobs are
identified by id so the file survives edits while a render runs.

## TUI keys

`n` new job · `j/k` move · `Enter` run selected · `r` next pending ·
`a` all pending · `x` cancel render · `d` delete · `p` re-queue ·
`R` reload queue · `q` quit

Renders run on a worker thread — the queue stays editable while a job
renders. Quitting mid-render SIGKILLs the child rather than orphaning a
half-written video.

## Engine

The `render` engine ([crate](engine/), bins `render` + `idle-render`)
runs a saver plugin's simulation headless and encodes frames to video
(AV1 default, H.264 for `--container mp4`), PNG sequences, or raw BGRA.
Studio drives it via `--job-file`.

```sh
render -e beams --duration 30s -o out.mkv              # AV1 in Matroska
render -e hearth --duration 5m --segment 1m -o h.mkv   # segmented + concat
render -e ripple --format png -o frames/               # one PNG per frame
render -e beams --stdout-raw | consumer                # raw BGRA + GBRI header
render --job-file job.json                             # JSON contract (Studio)
render -e beams --duration 1s --dry-run                # plan only, no encode
```

`idle-render` is the same binary under a namespaced name — installed
alongside `render` so `/usr/bin/render` collisions aren't fatal.

Notable flags: `--seed`, `--fps`, `--width/--height` (or `--cols/--rows`
for grid), `--crf` (0–63), `--preset`, `--encoder` (force an ffmpeg
encoder; auto-detection probes hardware first then falls back to
libsvtav1/libx264), `--no-hw-encode`, `--cpu-raster` (deterministic,
bypasses GPU variance), `--audio` (loop/shorten a bed to video length),
`--resume` (skip existing segment parts, still advances the sim),
`--baseline-dir` + `--snapshot-last-only` (byte-compare the last frame
for determinism CI — `--update-baselines` is gated behind
`RENDER_FORCE_UPDATE_BASELINES=1`).

Plugin resolution: `--plugin-path foo.so` loads an explicit saver
library (unsigned loads need `IDLE_ALLOW_UNSIGNED_PLUGINS=1`); otherwise
the effect name resolves through idle-runner's signed-manifest
discovery.

### Exit codes

| Code | Meaning |
|---|---|
| 0 | success |
| 1 | pipeline/encode error |
| 2 | argument-parse error |
| 3 | snapshot baseline missing |
| 4 | snapshot mismatch |

## Layout

The root crate is `idle-studio` (lib `idle_studio` + `idle-studio` bin);
`engine/` is the render engine member (lib `idle_render` + `render`/
`idle-render` bins). The build needs a sibling `runtime` checkout —
`./runtime` is a symlink or real checkout of `idlescreen/runtime`
(provides `idle-runner`, `idle-api`, the wayland/dbus crates). CI
creates it automatically; locally, `scripts/package.sh` symlinks
`../runtime` when present.

Packaging: signed RPM/DEB via release workflow; `scripts/package.sh`
runs the local gate (`qa_package_gate.sh`) then builds both crates into
the packages pool.
