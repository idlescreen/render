# IdleScreen Studio

**UI for the [render](../) capability** (same repository).

Studio does **not** encode video itself. It:

1. Queues jobs as render [`JobSpec`](../engine/src/job_spec.rs) JSON
2. Runs them with `render --job-file …`
3. Shows status in a small TUI

```text
you → studio (UI) → render (capability) → video file
```

### Commands

```bash
# from render repo root (symlink idle → ../idle if missing)
cargo build --release -p render -p idle-studio

# PATH or workspace binary
export PATH="$PWD/target/release:$PATH"
idle-studio enqueue -e ripple -o /tmp/ripple.mkv --duration 10s
idle-studio list
idle-studio run
idle-studio tui
```

Requires the `render` binary on `PATH` (or `target/release/render` in this workspace)
and IdleScreen saver plugins (e.g. `idle-savers` package → `/usr/libexec/idle/screensavers`).
