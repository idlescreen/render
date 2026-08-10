# AGENT.md — render (capability + studio UI)

## Product model

- **render** = offline export **capability** (library + `render` binary).
- **studio/** = **UI** that queues jobs and runs `render --job-file`.
- Upscale/draw/encode are **inside** the capability, not separate products.

## Contract

- Strict Rust, Apache-2.0.
- Max 256 lines per `.rs` file.
- Zero `.unwrap()` / `.expect()` in production code.
- Prefer `std`; vetted crates only (clap, serde, thiserror, tracing, proptest in dev).
- Job contract: `JobSpec` JSON + CLI. Protocol/parsing has proptest coverage.
- Default branch: master. Commit after each hardening barrier.

## Process kit

This repo follows the global process kit — local copies of the always-on process documents live next to this file:

- `AXIOMS.md` — always-on axioms (hygiene, security, entropy)
- `OODA.md` — Observe/Orient/Decide/Act rotation
- `PROBE.md` — assumption-hunt protocol

