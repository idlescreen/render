# idle-studio

Director for offline IdleScreen renders — queue export jobs, tune
parameters, and run them through the
[`render`](https://github.com/idlescreen/render) engine (saver math → AV1
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

## Build layout

`studio/` is a workspace member of the `render` repo (`idle_studio` lib +
`idle-studio` bin). It path-deps on the sibling `engine/` member; the only
external checkout needed is `../idle` at the repo root (CI creates it;
locally `bootstrap.sh` symlinks it).
