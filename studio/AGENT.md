# AGENT.md — studio (UI for render)

- Same repo as **render** (capability). Studio is UI only.
- Drive export via `render --job-file` / `idle_render::JobSpec` — do not reimplement encode.
- Strict Rust, Apache-2.0.
- Max 250 lines per `.rs` file.
- Zero `.unwrap()` / `.expect()` in production code.
- Default branch: master.
