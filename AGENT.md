# AGENT.md — render (capability + studio UI)

## Product model

- **render** = offline export **capability** (library + `render` binary).
- **studio/** = **UI** that queues jobs and runs `render --job-file`.
- Upscale/draw/encode are **inside** the capability, not separate products.

## Contract

- Strict Rust, Apache-2.0.
- Max 250 lines per `.rs` file.
- Zero `.unwrap()` / `.expect()` in production code.
- Prefer `std`; vetted crates only (clap, serde, thiserror, tracing, proptest in dev).
- Job contract: `JobSpec` JSON + CLI. Protocol/parsing has proptest coverage.
- Default branch: master. Commit after each hardening barrier.
