# Research: App Shell and Hardware Check

Findings come from general knowledge and the earlier web search. Items marked **verify** were not checked against live docs in this session; confirm them in the first implementation task.

## R1. Desktop shell
- **Decision**: Tauri 2.
- **Rationale**: Uses the OS web view, so the installer is a few MB and idle RAM is low (fits the low-spec principle). Rust core suits hardware probing and downloads. Tauri 2 also targets iOS and Android, which helps feature 010. Builds Windows installers and macOS app bundles.
- **Alternatives**: Electron (bundles Chromium, 100+ MB, heavier RAM; rejected for low-spec). Flutter desktop (good, but a second UI stack and no reuse of web UI). Native per platform (violates one-codebase and Principle I cost).
- **Risk**: OS web view differences (WebView2 on Windows, WKWebView on macOS). Mitigation: Playwright run on both, avoid bleeding-edge web APIs. **verify** WebView2 is present on Windows 10 1809; the installer can bootstrap it.

## R2. Hardware probing
- **Decision**: `sysinfo` for CPU cores, RAM, free disk, OS version; `wgpu` adapter listing for GPU name and backend (Metal, DX12, Vulkan).
- **Rationale**: Both are maintained, cross-platform, permissive. `wgpu` gives a uniform adapter list without vendor SDKs.
- **Limits**: VRAM is not reliably reported. The plan treats VRAM as unknown unless the platform gives it, and Apple Silicon unified memory counts as RAM. Unknown values never crash; they push toward hybrid (spec US1 scenario 6).
- **Alternatives**: Shelling out to `wmic`/`system_profiler` (fragile, parsing, `wmic` removed in newer Windows). Vendor tools (NVML only NVIDIA).

## R3. Spec definition (minimum and full-local)
- **Decision**: One JSON file `config/spec-definition.json` read by the core, the Windows installer hook and the docs generator. Provisional values:
  - Minimum: 64-bit CPU, 4 cores, 8 GB RAM, 10 GB free disk, Windows 10 1809+ or macOS 12+.
  - Full local: 8+ cores, 16 GB RAM, 40 GB free disk, and either an Apple Silicon chip or a discrete GPU reported by `wgpu`.
  - Hybrid: meets minimum but not full local.
- **Rationale**: Spec FR-003 demands a single source. Numbers are guesses until feature 002 benchmarks real pipeline steps; the file is marked `"provisional": true`.
- **Open question for owner**: 8 GB RAM minimum may exclude some of the "low spec" machines you want to reach. Lowering it to 4 GB needs proof that light steps fit; decide after benchmarks in 002.

## R4. Project file format
- **Decision**: Single `.flick` file = ZIP container with `manifest.json` (format version, name, timestamps, app version), `project.json` (feature data, empty for now), and room for later assets.
- **Rationale**: Portable, inspectable, one file, later features can add entries (transcript, audio references) without a new format. Written atomically (write temp, fsync, rename) with rolling backups, which meets crash recovery.
- **Alternatives**: SQLite single file (strong crash safety, but opaque, and big transcripts rewritten in place complicate version migration; can still be added inside the ZIP later). Plain folder (not a single file; rejected by FR-010).
- **Unicode**: file names inside the ZIP are UTF-8; project name lives in the manifest, not the file name, so non-Latin names are safe on both platforms.
- **Locking**: a `.lock` sidecar with process ID. Second opener goes read-only.
- **Autosave/recovery**: recovery copy written every 30 s while dirty, giving the 60 s bound in SC-005.

## R5. Install-time check
- **Decision**: First launch is the authoritative check on both platforms. On Windows, the installer (NSIS, via Tauri bundler hook) runs `flickfluent --check-hardware` and aborts with the message if blocked. On macOS, drag-and-drop DMG installs cannot run code, so the alert shows at first launch.
- **Rationale**: Gives the owner's "alert on installation" wherever the platform allows, and never lets a blocked machine proceed.
- **Note for owner**: macOS cannot truly block at install time. This is a platform limit, not a choice.
- **verify**: exact NSIS hook syntax in the current Tauri 2 bundler.

## R6. Model downloads
- **Decision**: `reqwest` streaming with HTTP Range for resume, writing to `.part` files, SHA-256 verification against the catalogue, free-space check before start, and a JSON index of installed models.
- **Rationale**: Covers pause, resume, interruption and corruption (US4). Catalogue entries carry name, purpose, size, licence, URL, SHA-256.
- **Alternatives**: Hugging Face Hub client (adds a Python dependency now; also ties us to one host). Bundling models in the installer (breaks the small-install goal).
- **Edge**: servers without Range support restart from zero; the UI says so.

## R7. Settings, hotkeys, theme, i18n
- **Decision**: JSON settings file in the OS config folder, written atomically. Theme via CSS variables with light, dark and follow-system. Hotkeys stored as a map with conflict detection in core. UI text from `i18n/en.json` via `i18next`.
- **Rationale**: Simple, testable, meets FR-013 to FR-015 and FR-020.

## R8. Dependency licences
- **Decision**: Use only MIT or Apache-2.0 components. Tauri (MIT/Apache-2.0), sysinfo (MIT), wgpu (MIT/Apache-2.0), reqwest (MIT/Apache-2.0), sha2 (MIT/Apache-2.0), zip (MIT), React (MIT), Vite (MIT), i18next (MIT). **verify** each at implementation time with a licence-check tool in CI (`cargo deny`, `license-checker`).
- **Rationale**: Principle VI.

## R9. Memory indicator
- **Decision**: Core samples the app's own memory and system memory every 2 s with `sysinfo` and emits an event; UI shows a small badge with a warning level at 85% system memory.
- **Alternatives**: Polling from the UI (costs more and differs per web view).

## R10. Testing across two platforms
- **Decision**: GitHub Actions matrix `windows-latest` and `macos-latest` running `cargo test`, Vitest and Playwright. The repository is not under git yet, so CI config is prepared but cannot run until the project is pushed.
- **Risk**: No physical low-spec test machine. Mitigation: the hardware probe takes an injectable "fake machine" input so the three outcomes are testable on any computer.

## T008 verification results (2026-11-04)

Checked with web search and `cargo metadata`; not run on Windows.

- **NSIS hook syntax**: confirmed. Config key is `bundle.windows.nsis.installerHooks` (path to a `.nsh` file); macros are written `!macro NSIS_HOOK_POSTINSTALL ... !macroend`; `NSIS_HOOK_POSTINSTALL` runs after files are copied. The hook in `src-tauri/installer/check-hardware.nsh` follows this. Its uninstall-on-block steps are still **untested on Windows** (task T058).
- **WebView2**: Tauri 2 offers a `minimumWebview2Version` installer setting. No source found that names Windows 10 1809 as the minimum. Spec definition therefore uses "Windows 10 or newer" (major version only), not a build number.
- **macOS minimum**: not confirmed for Tauri 2 (only v1 docs say 10.13; one mirror says 10.15). The spec uses macOS 12, which is stricter than any of these, so it is safe. Set `bundle.macOS.minimumSystemVersion` to `12.0` at packaging time (open item for T058).
- **Licences** (from `cargo metadata`, 600+ crates): all permissive except these, none a blocker:
  - MPL-2.0 (file-level copyleft, allowed with notice): cssparser, cssparser-macros, dtoa-short, option-ext, selectors. All are transitive dependencies of Tauri.
  - `r-efi`: offered as MIT OR Apache-2.0 OR LGPL; we use it under MIT or Apache-2.0.
  - `webpki-roots`: CDLA-Permissive-2.0 (permissive).
  - No GPL or AGPL, no non-commercial licences.
  - npm packages not yet checked; that runs in T060.
