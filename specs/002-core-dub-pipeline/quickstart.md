# Quickstart: validating feature 002

Run these after implementation. Details of shapes are in `data-model.md` and `contracts/`.

## Prerequisites
- Windows 10+ or macOS 12+, free disk at least 15 GB, internet for the first model downloads.
- Repo built as in feature 001 (`npm install`, `cargo build`).
- Fixtures present in `fixtures/` (20 s clip with two speakers, a music-only clip, a silent-video clip, a corrupt file, a 2-minute clip).

## 1. Stage contract tests (test-first, both platforms)
```sh
cargo test --workspace                           # stage contracts, resume, install, preflight (fake sidecar)
FLICK_TEST_MODELS=<models> python -I -m pytest sidecar/tests   # real workers on the 20 s fixture (skipped without models)
FLICK_TEST_MODELS=<models> FLICK_TEST_PYTHON=<python> cargo test -p flickfluent-core --test pipeline_real -- --ignored   # whole dub, real models
```
Expected: all stage rows in `contracts/stage-contracts.md` pass, including the interrupt test per stage.

## 2. Languages from data
```sh
cargo test -p flickfluent-core --test languages
```
Expected: the list equals the union of catalogue entries. Adding a test catalogue entry with a new language code makes it appear with no UI change (SC-005). An unsupported step returns a `PairCheck` with a fallback (SC-006).

## 3. End-to-end dub (2-minute clip)
1. Start the app (`npm run tauri dev`), open New dub, choose `fixtures/clip-2min-fr.mp4`.
2. Set source `auto-detect`, target `English`. The detected language shows after the run starts.
3. The preflight lists missing models with sizes; approve the downloads.
4. Press start. Stage name, percentage and ETA update at least every 5 s.
5. Export full video, then export audio only.

Expected: dubbed English speech, background music audible, speech lines up with scenes, both files play in the OS player. Time from start to finished export is under 10 minutes on a minimum-spec machine (SC-001).

## 4. Interrupt and resume
1. Start a run on the 2-minute clip. During `transcribe`, kill the app process.
2. Reopen the project. The app offers resume.
3. Resume. Only the interrupted section reruns; finished sections are untouched (compare file timestamps) (SC-003).

## 5. Long movie
Use a generated 3-hour file (script in `fixtures/make-long.sh`, looped clip). Run on a minimum-spec or limited-RAM machine. Expected: completes, peak memory under the machine-class ceiling (SC-002). Record the memory curve.

## 6. Errors
- Delete the source file mid-run: error names the stage and gives a next step (SC-009).
- Fill the disk (small test volume): clean stop, finished work kept, exact space needed shown.
- Pick source = target: confirmation asked.
- Select a language with no permissive voice: warning and fallbacks appear before start.

## 7. Benchmark and spec values
```sh
flickfluent --bench --json > bench-<machine>.json
```
Run on at least one minimum-class and one full-local machine per OS. Expected: records saved; `config/spec-definition.json` updated from them, `"provisional"` removed, schema test passes; `docs/verification/002-benchmarks.md` lists every number's source (SC-010). Re-run the hardware check on each machine: the chosen mode matches the record's verdict.

## 8. Licence check
`docs/dependencies.md` lists every new component with licence. Any non-permissive entry shows a notice in the model list and in About. Run the CI licence job.

## 9. Cross-platform parity
Run steps 1 to 6 on Windows and macOS; fill `docs/verification/002.md`. Feature 002 is done only when both platforms pass (SC-007). Windows is still open from feature 001.
