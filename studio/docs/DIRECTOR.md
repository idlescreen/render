# Director notes

Studio does not re-implement the saver math. It:

1. Collects creative parameters.
2. Invokes `render` (or links its library) per job.
3. Tracks progress, failures, and output paths.
4. Optionally muxes audio and writes channel metadata.
