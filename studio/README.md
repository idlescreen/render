# IdleScreen Studio

**TUI-first Director** for the [render](../) capability (same repository).

Studio does **not** encode video itself. It:

1. Queues jobs as render [`JobSpec`](../engine/src/job_spec.rs) JSON  
2. Runs them with `render --job-file …`  
3. Lets you manage the queue in a **full-screen TUI**

```text
you → idle-studio (TUI) → render → video file
```

## Primary use: open the Director

```bash
# from render repo root
cargo build --release -p render -p idle-studio
export PATH="$PWD/target/release:$PATH"

# No subcommand → TUI
idle-studio

# Same as:
idle-studio tui
```

### TUI keys

**Queue screen**

| Key | Action |
|-----|--------|
| `n` | New job form |
| `j` / `k` | Move selection |
| `Enter` | Run selected job |
| `r` | Run next pending |
| `a` | Run all pending |
| `d` | Delete selected |
| `p` | Mark selected pending again |
| `R` | Reload queue from disk |
| `q` | Quit |

**New job form**

| Key | Action |
|-----|--------|
| `Tab` / `j` / `k` | Next / previous field |
| `h` / `l` | Cycle effect or toggle dry_run |
| `Enter` | Edit text field / cycle effect / toggle dry_run |
| type… | When editing: change value |
| `s` | Save job to queue |
| `Esc` | Cancel back to queue |

Queue file default: `~/.config/idle-studio/queue.json`  
Requires **`render` on PATH** (or `target/release/render`) and saver plugins  
(`idle-savers` → `/usr/libexec/idle/screensavers`).

## Scripting (optional)

CLI subcommands remain for automation:

```bash
idle-studio enqueue -e ripple -o /tmp/ripple.mkv --duration 10s
idle-studio list
idle-studio run
idle-studio run --all
```
