# Dependencies and licences

Principle V (reuse open source) and Principle VI (licence discipline). Every reused component is listed here. Licences marked "to verify" are checked in task T008 and T060.

| Component | Purpose | Licence | Why chosen |
|-----------|---------|---------|------------|
| Tauri 2 | Desktop shell, installer bundling, later mobile | MIT OR Apache-2.0 | Small install, low RAM, one codebase for Windows and macOS (Linux and mobile later) |
| tauri-plugin-single-instance | One app window | MIT OR Apache-2.0 | Official plugin |
| sysinfo | CPU, RAM, disk, process memory | MIT | Cross-platform, maintained |
| wgpu | GPU adapter listing only | MIT OR Apache-2.0 | One API for Metal, DX12, Vulkan |
| reqwest | Model downloads | MIT OR Apache-2.0 | Standard Rust HTTP client with streaming |
| tokio | Async runtime | MIT | Needed by reqwest |
| sha2 | SHA-256 for model integrity | MIT OR Apache-2.0 | RustCrypto |
| zip | `.flick` project container | MIT | Plain ZIP, portable |
| serde, serde_json | Data formats | MIT OR Apache-2.0 | Standard |
| thiserror, uuid, time | Errors, ids, timestamps | MIT OR Apache-2.0 | Standard |
| jsonschema (test only) | Validate spec-definition.json | MIT | Test-only |
| React, React DOM | UI | MIT | Large ecosystem, plain web code reusable on web and mobile |
| Vite, Vitest | Build and unit tests | MIT | Standard |
| i18next | UI text catalogue | MIT | Keeps text separable (FR-020) |
| Playwright | End-to-end tests | Apache-2.0 | Cross-platform |
| ESLint, Prettier, TypeScript, @types/node | Lint, format, types | MIT / MIT / Apache-2.0 / MIT | Standard |
| tauri-plugin-dialog, @tauri-apps/plugin-dialog | Open and save file pickers | MIT OR Apache-2.0 | Official Tauri plugin |
| tempfile (test only) | Temporary folders in tests | MIT OR Apache-2.0 | Test-only |
| FFmpeg (LGPL build, BtbN FFmpeg-Builds on Windows) | Read, cut, mix and mux media | LGPL-2.1-or-later | Only practical tool for any common format. Run as a separate program, downloaded on demand, replaceable. Notice in About. No macOS LGPL build is pinned yet (see below). |
| faster-whisper | Speech to text | MIT | Fast CPU inference through CTranslate2 |
| CTranslate2 | Inference engine for Whisper and M2M100 | MIT | int8 CPU speed, low memory |
| M2M100 (418M, 1.2B) | Translation, about 100 languages | MIT | Permissive weights. NLLB-200 (CC-BY-NC-4.0) is a separate opt-in entry, off by default, with a licence notice. |
| Demucs (htdemucs), fork adefossez/demucs | Separate voice from music | MIT code; weights covered by the README statement (owner decision 2026-11-04, see research.md) | Windowed processing keeps memory flat |
| sherpa-onnx | CPU text to speech runtime | Apache-2.0 | Runs Piper and Kokoro voices without the GPL Piper engine |
| Piper voices | Voices for text to speech | Per voice, checked in its model card | Voice files only; the archived and GPL engine is not used. A voice with a non-permissive licence is not listed by default. Catalogue voices: LJ Speech (public domain), Alba and Siwis (CC-BY-4.0), Thorsten and DaveFX (CC0). Most Piper voices are fine-tuned from the Lessac voice, whose dataset licence is restrictive (the Lessac voice itself is left out); whether fine-tuned weights inherit it is not settled. |
| eSpeak NG (pronunciation data and library inside sherpa-onnx) | Turns text into sounds for Piper voices | **GPL-3.0-or-later** | Piper voices need it. See the licence note below. |
| Kokoro-82M | Voice for text to speech | Apache-2.0 | Good quality for the languages it covers |
| silero-vad | Find speech and silence | MIT | Small, accurate |
| python-build-standalone (CPython 3.11) | Python runtime pack | PSF-2.0 (CPython), MIT-style build scripts | Downloaded on first dub, never in the installer (25 MB budget). Pinned release 20261026 with SHA-256 in the catalogue. |

## Licence notes (checked 2026-11-04 with `cargo metadata`)

No GPL, AGPL or non-commercial licence is used in feature 001. Exceptions to "MIT or Apache-2.0" among the Rust crates, all transitive dependencies of Tauri:

| Crates | Licence | Note |
|--------|---------|------|
| cssparser, cssparser-macros, dtoa-short, option-ext, selectors | MPL-2.0 | File-level copyleft. Used unmodified. Source of these crates must stay available; keep the licence notice in the app's About screen. |
| webpki-roots | CDLA-Permissive-2.0 | Permissive |
| r-efi | MIT OR Apache-2.0 OR LGPL-2.1-or-later | Used under MIT or Apache-2.0 |

npm packages are checked by the CI licence job and task T060.

## Feature 002 notes (2026-11-04)

- **Pinned in the catalogue**: python-build-standalone `20261026` (CPython 3.11.18, `install_only_stripped`) for Windows and macOS on x64 and arm64; FFmpeg `n8.1.3-34-ga35157abb9` LGPL builds for Windows x64 and arm64 from BtbN release `autobuild-2026-11-04-12-51`. Sizes and SHA-256 come from the release pages.
- **Open: macOS FFmpeg.** No trustworthy prebuilt LGPL macOS build was found. Run the `ffmpeg-macos` workflow (`.github/workflows/ffmpeg-macos.yml`), upload its archives to the project release, and add the two macOS `ffmpeg` entries with the printed size and SHA-256.
- **Open: BtbN retention.** Autobuild releases are pruned over time. Before release, mirror the pinned archives to storage we control and update the URLs.
- **Not yet in the catalogue**: faster-whisper, M2M100, Demucs, sherpa-onnx, Piper, Kokoro and silero-vad model files. Their licences and hashes are verified when each story adds its entry (licences are logged in `specs/002-core-dub-pipeline/research.md`; sizes and hashes are written by T041).

## Licence note: eSpeak NG (found 2026-11-04)

Piper voices run through sherpa-onnx, and sherpa-onnx uses eSpeak NG (GPL-3.0) for pronunciation. The research notes assumed only the voice files were involved. What this means here:

- The pronunciation data is its own catalogue entry (`espeak-ng-data`, class `copyleftFile`, notice key `licence.espeak`), so it can be shown and replaced.
- The sherpa-onnx wheel in the runtime pack contains eSpeak NG code. The sidecar is a separate process that the app talks to over stdin and stdout, and the pack is a download, not part of the installer.
- Whether that keeps the app itself free of GPL obligations is **an open question for the owner**. If it does not, the options are to drop Piper voices and use only voices that do not need eSpeak (Kokoro with its own lexicon, MMS), or to publish the matching source for the pack.

## Package lists (2026-11-04)

`sidecar/locks/` holds hashed package lists for the runtime pack, made with `uv pip compile --generate-hashes --only-binary :all:`: `macos-arm64` (full), `windows-x64` (full), `macos-x64` (no `demucs`/`torch`: `sphn` has no wheel). Windows on ARM has none: `ctranslate2` has no wheel. `sherpa-onnx` is pinned below 2 because 2.0.1 cannot phonemize for Piper voices. Installing a pack runs `pip install --require-hashes --only-binary=:all: --no-deps`, so nothing is built or swapped on the way. Packages are fetched from PyPI at install time and the pack is verified by hash like any download (about 0.75 GB with `torch`, 0.2 GB on Intel Macs). The catalogue records the packages as `extraDownloadBytes`, and the size shown to the user is the pack plus the packages.
