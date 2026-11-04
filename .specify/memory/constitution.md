# FlickFluent Constitution

FlickFluent translates and dubs a short or long movie from one language into any other. It transcribes speech, translates it, clones the original voices, separates voice from background music, and exports a dubbed video with subtitles. Its feature set is modelled on the "Professor Storytelling – AI Video & Voice Studio PRO" tool shown in the reference video in this directory.

## Core Principles

### I. Cross-Platform: Windows and macOS first, then Linux, web and mobile (NON-NEGOTIABLE)
Every feature MUST work on both Windows and macOS. No feature ships if it works on only one. Linux, web and mobile are required targets that follow in later releases (amended 2026-11-04 at the owner's request); once a platform ships, every feature on it follows the same rule. Hardware-heavy features (lip-sync, real-time dubbing) MAY rely on the cloud option on devices that cannot run them, per Principle II. Platform-specific code lives behind a shared interface, and CI or manual verification MUST cover both platforms before a feature is done. Apple-only (MLX, Core ML) or Windows-only (DirectML, CUDA-only) accelerators MAY be used only as optional speed-ups with a working fallback on the other platform.

### II. Runs on Low-Spec Machines (NON-NEGOTIABLE)
The app MUST be usable on a low-spec computer. Heavy work degrades gracefully instead of failing:
- Light steps always run locally with small models: audio extraction (ffmpeg), transcription (faster-whisper, model size chosen by machine), and vocal/music separation (Demucs).
- Heavy steps (translation, voice cloning) run locally only if the machine can handle them; otherwise they use an optional cloud API with the user's own key.
- Long movies are processed in chunks with a bounded memory ceiling. A whole movie is never loaded into RAM at once.

### III. Install-Time Hardware Check and Alert (NON-NEGOTIABLE)
The installer and first launch MUST check CPU, RAM, free disk, and GPU/accelerator against the minimum spec, and select a mode (full local, hybrid, or cloud-assisted). If the machine is below the minimum, the app MUST show a clear alert and stop, for example: "Your computer is not compatible with this app. You need xyz spec to run this app." The alert MUST name the exact missing requirement. The minimum spec is defined once, in one place, and used by the check, the alert and the docs.

### IV. Any Language from a Dropdown (NON-NEGOTIABLE)
Source and target language MUST be chosen from a dropdown. Adding a language MUST NOT need a code change in the UI. The language list is data-driven from the capabilities of the installed or selected models. If a chosen language is not supported by a step (for example no voice cloning for that language), the app MUST say so before processing and offer a fallback.

### V. Reuse Open Source; Do Not Reinvent the Wheel (NON-NEGOTIABLE)
Before writing any component, check for a maintained open-source tool that does it. Build only glue, UI and orchestration. Each reused tool is recorded in `docs/dependencies.md` with its purpose, version, licence and why it was chosen.

### VI. Licence Discipline
Commercial-friendly licences (MIT, Apache-2.0, BSD) are preferred. Non-commercial or share-alike licences (CC-BY-NC, CC-BY-NC-SA, GPL/AGPL) MAY be used only if no viable permissive option exists, and only with: a recorded justification in `docs/dependencies.md`, the component isolated so it can be replaced, and a user-visible licence notice. Personal use is the baseline; commercial use MUST NOT be silently blocked or silently assumed.

### VII. Voice Cloning Consent
Voice cloning is used only on audio the user supplies and for dubbing the user's own project. The app MUST show a short notice that the user is responsible for having the right to clone and dub the voices and the movie.

### VIII. Test-First for Core Pipeline
The pipeline stages (extract, separate, transcribe, translate, synthesize, mux, subtitle) each have a contract test on a short sample clip before implementation. A stage is done only when its test passes on both platforms.

### IX. Privacy and Local-First
Media stays on the user's machine unless the user has turned on a cloud step. Cloud steps are off by default, labelled in the UI, and show what will be sent. API keys are stored in the OS keychain, never in plain text or in the repo.

## Technology Constraints

- Stack (provisional, confirm in the plan): a desktop shell (Tauri) with a Python sidecar for the media pipeline.
- Defaults: ffmpeg, faster-whisper, Demucs, NLLB or M2M100 for translation, a permissively licensed cloning TTS (Chatterbox or VoxCPM2 candidates).
- Every model is downloaded on demand, with a size shown before download.

## Development Workflow

- All work goes through speckit: constitution, then specify, plan, tasks, implement.
- Every plan MUST include a Constitution Check against Principles I to IX. A violation is a blocker unless the constitution is amended first.
- Reverse-engineering the reference video is limited to behaviour and UI structure. No assets, code or branding of the reference tool are copied.

## Governance

This constitution supersedes all other practices. Principles marked NON-NEGOTIABLE can be changed only by an explicit written amendment from the project owner, with the reason recorded here. All plans, tasks and reviews MUST verify compliance. Complexity MUST be justified.

**Version**: 1.1.0 | **Ratified**: 2026-11-04 | **Last Amended**: 2026-11-04
