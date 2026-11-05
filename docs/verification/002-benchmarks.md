# Feature 002 benchmarks

Measured with `flickfluent --bench` on the 2-minute French clip (`fixtures/clip-2min-fr.mp4`), dubbed into English. Peak memory is per stage: the benchmark restarts the worker process after every call (`ramCeilingMb: 1`), so a stage is not charged for what an earlier one loaded.

**Status: the spec is still provisional (`"provisional": true`, `"basis": "simulated"`).** There are records from one operating system only, and the minimum spec was simulated, not run on a real 8 GB computer. Windows has no record at all.

## Records

| Record | Machine | Simulated | Used for |
|--------|---------|-----------|----------|
| `bench-macos-arm64-real-20261105-c887f623.json` | macOS, Apple M5, 10 cores, 16 GB, Apple Silicon | no | full-local numbers |
| `bench-macos-arm64-simulated-20261105-e5f34e2e.json` | same Mac held to 4 cores and 8 GB | **yes** | minimum numbers |
| Windows, any | not run | n/a | missing (task T075, needs a Windows computer) |

## Results (2-minute clip, real-time factor = processing time / media time)

| Stage | Full-local Mac: model | wall | peak RAM | RTF | Simulated minimum: model | wall | peak RAM | RTF |
|-------|-----|-----|-----|-----|-----|-----|-----|-----|
| extract | ffmpeg | 0.1 s | n/a | 0.001 | ffmpeg | 0.2 s | n/a | 0.001 |
| separate | htdemucs | 41.2 s | 1789 MB | 0.343 | htdemucs | 49.7 s | 1642 MB | 0.414 |
| transcribe | whisper-large-v3 | 519.3 s | 4066 MB | 4.323 | whisper-small | 22.6 s | 1161 MB | 0.188 |
| translate | m2m100-1.2b | 11.1 s | 2761 MB | 0.092 | m2m100-418m | 3.0 s | 1574 MB | 0.025 |
| synthesize | kokoro-multi-lang-v1_0 | 11.8 s | 836 MB | 0.099 | kokoro-multi-lang-v1_0 | 13.0 s | 711 MB | 0.108 |
| mix | ffmpeg | 0.2 s | n/a | 0.002 | ffmpeg | 0.2 s | n/a | 0.002 |
| **whole dub** | | | **4066 MB** | **4.86** | | | **1642 MB** | **0.74** |

The watchdog in `bench/simulate-limits.sh` saw at most 1649 MB resident for the whole simulated run (all processes together), against its 8192 MB limit.

Things to know before trusting these:

- **Whisper large-v3 is slow on the CPU.** The same model took 14 s for the 20-second clip (RTF 0.70) and 519 s for the 2-minute clip (RTF 4.32). That is not linear, and it was run once. A repeat, ideally on a quiet machine, is needed before anyone plans around it. The full-local class picks large-v3, so a long movie on the full-local class may be much slower than the small model on the minimum class.
- **The simulation is a best case.** The simulated run uses the same fast M5 cores, with the worker given 3 threads, and no operating system or other apps competing for memory. Speeds from it are a ceiling; memory numbers are real.
- **Peak RAM is the worker's own.** It leaves out the app window and the core process (about 150 MB idle, see 001).
- **One clip, one language pair.** French to English. Other pairs use other models (Khmer would use NLLB or M2M100 and a different voice) and were not measured.

## Spec values

Each value below is the one in `config/spec-definition.json` (a test checks they match). The last column says what supports it.

| Value | Setting | Supported by |
|-------|---------|--------------|
| minimum.cpuBits | 64 | the runtime pack only exists for 64-bit systems |
| minimum.cpuCores | 4 | `bench-macos-arm64-simulated-20261105-e5f34e2e.json` ran the minimum models with 3 worker threads; fewer cores were not tried |
| minimum.ramGb | 8 | `bench-macos-arm64-simulated-20261105-e5f34e2e.json`: heaviest stage 1642 MB. See the 8 GB decision below |
| minimum.freeDiskGb | 10 | catalogue: minimum-class models about 2.4 GB plus the runtime pack up to 0.75 GB plus a work folder of about 0.6 GB per 10 minutes of video; not measured on a full film |
| minimum.os.windows | 10 | not measured (no Windows record) |
| minimum.os.macos | 12 | not measured (the Mac used runs a newer system) |
| minimum.accelerator | none | `bench-macos-arm64-simulated-20261105-e5f34e2e.json` used no accelerator for the models |
| fullLocal.cpuBits | 64 | as minimum |
| fullLocal.cpuCores | 8 | `bench-macos-arm64-real-20261105-c887f623.json` had 10 cores; fewer were not tried |
| fullLocal.ramGb | 16 | `bench-macos-arm64-real-20261105-c887f623.json`: heaviest stage 4066 MB; the Mac had exactly 16 GB |
| fullLocal.freeDiskGb | 40 | catalogue: all mid and full-local models about 6 GB; not measured on a full film |
| fullLocal.os.windows | 10 | not measured (no Windows record) |
| fullLocal.os.macos | 12 | not measured |
| fullLocal.accelerator | appleSilicon_or_discreteGpu | `bench-macos-arm64-real-20261105-c887f623.json` ran on Apple Silicon; the models used run on the CPU, so the accelerator is not what made it work. A discrete GPU was not tried |

## Does 8 GB RAM stay?

**Yes, it stays at 8 GB.** The owner's rule is to lower it only if light steps are proven to fit on less. The simulated run needed 1.6 GB for the heaviest stage, so the models themselves would fit on much less than 8 GB. But the proof is missing in three ways: nothing was run on a computer with less memory, the operating system and other apps share that memory on a real machine, and Windows was not run. Until a real low-memory computer (Windows and Mac) completes the 2-minute clip, 8 GB stays.

## What is still open

- Run `flickfluent --bench` on Windows, real and simulated (`bench/simulate-limits.ps1`, never executed), and save the records here. Until then the spec stays provisional.
- Run on a real computer with 8 GB.
- Repeat the Whisper large-v3 run to explain its non-linear time.
- Run a full-length film (the 3-hour run, SC-002).
- Measure a non-Latin pair such as English to Khmer.

## How to reproduce

```
flickfluent --bench --json                   # real run, saves a record in the app data folder
bench/simulate-limits.sh                     # macOS, held to the minimum spec
powershell -File bench\simulate-limits.ps1  # Windows, not yet run
```

Add `--download` the first time to fetch the models the machine class needs. Copy the saved `bench-*.json` into `docs/verification/benchmarks/`.
