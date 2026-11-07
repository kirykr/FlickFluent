# Feature 002 benchmarks

Measured with `flickfluent --bench` on the 2-minute French clip (`fixtures/clip-2min-fr.mp4`), dubbed into English. Peak memory is per stage: the benchmark restarts the worker process after every call (`ramCeilingMb: 1`), so a stage is not charged for what an earlier one loaded.

**Status: the spec is still provisional (`"provisional": true`, `"basis": "simulated"`).** Both operating systems now have records, but the minimum spec was simulated on both, never run on a real 8 GB computer, and no Windows machine that meets full-local has been measured.

## Records

| Record | Machine | Simulated | Used for |
|--------|---------|-----------|----------|
| `bench-macos-arm64-real-20261105-c887f623.json` | macOS, Apple M5, 10 cores, 16 GB, Apple Silicon | no | full-local numbers |
| `bench-macos-arm64-simulated-20261105-e5f34e2e.json` | same Mac held to 4 cores and 8 GB | **yes** | minimum numbers |
| `bench-windows-x64-real-20261107-80c59d44.json` | Windows 11 Pro (22631), Intel i7-6700 (2015), 4 cores / 8 threads, 16 GB, GTX 1060 3 GB | no | Windows minimum-class numbers on real hardware (4 cores is below full-local, so the app picks the minimum models) |
| `bench-windows-x64-simulated-20261107-3531463a.json` | same PC held to 4 physical cores and 8 GB by a Job Object (`bench/simulate-limits.ps1`) | **yes** | Windows minimum numbers |
| Windows full-local | not run | n/a | missing: needs a Windows PC with 8 or more cores |

A first Windows real run (`...-3e63af8e`, not kept) recorded 0 MB for every stage: the worker's memory reading was broken on Windows (`peak_ram_mb` called the Win32 API through ctypes without declared types, so the call failed silently), and the verdict counted 0 MB as fitting. Both are fixed: the worker reads its peak working set, and a stage that reports no memory now gives `null` and a verdict that proves nothing. The two records above come from the fixed build.

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

### Windows (i7-6700, 2-minute clip, same models on both runs)

| Stage | Model | Real: wall | peak RAM | RTF | Simulated minimum: wall | peak RAM | RTF |
|-------|-------|-----|-----|-----|-----|-----|-----|
| extract | ffmpeg | 0.8 s | n/a | 0.007 | 0.8 s | n/a | 0.006 |
| separate | htdemucs | 162.3 s | 1158 MB | 1.351 | 142.3 s | 1155 MB | 1.185 |
| transcribe | whisper-small | 50.6 s | 982 MB | 0.421 | 44.3 s | 688 MB | 0.369 |
| translate | m2m100-418m | 7.8 s | 1581 MB | 0.065 | 7.1 s | 1581 MB | 0.059 |
| synthesize | kokoro-multi-lang-v1_0 | 28.0 s | 640 MB | 0.233 | 23.2 s | 556 MB | 0.193 |
| mix | ffmpeg | 1.3 s | n/a | 0.010 | 1.3 s | n/a | 0.010 |
| **whole dub** | | | **1581 MB** | **2.10** | | **1581 MB** | **1.84** |

The simulated run held the program and every process it started to an 8 GB job memory limit; Windows would have stopped it on going over, and it finished.

- **Voice separation is the slow step on an older CPU.** htdemucs took 162 s for 2 minutes (RTF 1.35), against 41 s on the M5. It is two thirds of the whole dub. A 2-hour film at this rate needs about 4 hours on this PC, most of it separation.
- **The simulated run was faster than the real one.** Both gave the worker 3 threads. The simulation pins the job to one logical processor per physical core (mask `0x55`), so the threads never share a core with a hyper-threading sibling; the real run let Windows place them freely. It was run once each, so part of the gap may be noise, but it suggests pinning worker threads to physical cores could help on hyper-threaded CPUs.
- **Memory matches the Mac closely** (heaviest stage 1581 MB here, 1642 MB on the simulated Mac), so the minimum models fit in 8 GB with a wide margin on both systems.
- **No GPU was used.** The GTX 1060 is not an accelerator the spec counts, and all models ran on the CPU.

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
| minimum.os.windows | 10 | `bench-windows-x64-simulated-20261107-3531463a.json` ran on Windows 11; Windows 10 itself was not tried |
| minimum.os.macos | 12 | not measured (the Mac used runs a newer system) |
| minimum.accelerator | none | `bench-macos-arm64-simulated-20261105-e5f34e2e.json` used no accelerator for the models |
| fullLocal.cpuBits | 64 | as minimum |
| fullLocal.cpuCores | 8 | `bench-macos-arm64-real-20261105-c887f623.json` had 10 cores; fewer were not tried |
| fullLocal.ramGb | 16 | `bench-macos-arm64-real-20261105-c887f623.json`: heaviest stage 4066 MB; the Mac had exactly 16 GB |
| fullLocal.freeDiskGb | 40 | catalogue: all mid and full-local models about 6 GB; not measured on a full film |
| fullLocal.os.windows | 10 | not measured (no Windows full-local machine; the i7-6700 has 4 cores) |
| fullLocal.os.macos | 12 | not measured |
| fullLocal.accelerator | appleSilicon_or_discreteGpu | `bench-macos-arm64-real-20261105-c887f623.json` ran on Apple Silicon; the models used run on the CPU, so the accelerator is not what made it work. A discrete GPU was not tried |

## Does 8 GB RAM stay?

**Yes, it stays at 8 GB.** The owner's rule is to lower it only if light steps are proven to fit on less. The simulated runs needed about 1.6 GB for the heaviest stage on both systems, so the models themselves would fit on much less than 8 GB. But the proof is still missing in two ways: nothing was run on a computer with less memory, and the operating system and other apps share that memory on a real machine. Until a real low-memory computer (Windows and Mac) completes the 2-minute clip, 8 GB stays.

## What is still open

- Run on a real computer with 8 GB. Until then the spec stays provisional.
- Run on a Windows PC that meets full-local (8 or more cores).
- Repeat the Whisper large-v3 run to explain its non-linear time.
- Run a full-length film (the 3-hour run, SC-002).
- Measure a non-Latin pair such as English to Khmer.

## How to reproduce

```
flickfluent --bench --json                   # real run, saves a record in the app data folder
bench/simulate-limits.sh                     # macOS, held to the minimum spec
powershell -File bench\simulate-limits.ps1 -BenchArgs '--json'   # Windows, held to the minimum spec; output in -Out
```

Add `--download` the first time to fetch the models the machine class needs. Copy the saved `bench-*.json` into `docs/verification/benchmarks/`.
