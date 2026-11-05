# Release checklist

Steps that are done by hand at release time. Add to this list; do not skip a step.

## Engine pack (runtime) release

- [ ] Publish the matching source code of the eSpeak NG and sherpa-onnx versions inside the pack, next to the pack on the project release page (GPL-3.0 and Apache-2.0 respectively). The notices `licence.espeak` and `licence.runtimePack` promise this at https://github.com/kirykr/FlickFluent/releases. Owner decision 2026-11-05, see `docs/dependencies.md`.
- [ ] Publish the matching source of `lameenc` (LGPL-3.0) the same way.
- [ ] Re-run the licence audit (`cargo deny check licenses`, the npm lockfile check, the Python metadata check) and update `docs/dependencies.md`.

## FFmpeg

- [ ] Run `.github/workflows/ffmpeg-macos.yml` and mirror the archives (BtbN deletes old Windows builds).

## Both platforms

- [ ] Run the full quickstart on Windows and macOS (`docs/verification/002.md`).
- [ ] Run `flickfluent --bench` on Windows, real and simulated, and save the records (`docs/verification/002-benchmarks.md`).
- [ ] Measure idle RAM and installer size on the release build.
