//! Every dub error code with the i18n keys for its message and its next step.
//! `contracts/ipc-commands.md` lists the codes. A test checks each key exists in `en.json`.

use crate::error::*;

/// `(code, message key, next-step key)`.
pub const DUB_ERRORS: &[(&str, &str, &str)] = &[
    (
        UNREADABLE_FILE,
        "dub.error.unreadableFile",
        "dub.next.chooseAnotherFile",
    ),
    (
        NO_AUDIO_TRACK,
        "dub.error.noAudioTrack",
        "dub.next.chooseAnotherFile",
    ),
    (
        NO_SPEECH_FOUND,
        "dub.error.noSpeechFound",
        "dub.next.exportOriginal",
    ),
    (
        SAME_LANGUAGE,
        "dub.error.sameLanguage",
        "dub.next.confirmOrChange",
    ),
    (
        NO_FALLBACK_CHOSEN,
        "dub.error.noFallbackChosen",
        "dub.next.chooseFallback",
    ),
    (
        MODELS_MISSING,
        "dub.error.modelsMissing",
        "dub.next.downloadModels",
    ),
    (NO_SPACE, "dub.error.noSpace", "dub.next.freeDiskSpace"),
    (
        MACHINE_TOO_WEAK,
        "dub.error.machineTooWeak",
        "dub.next.seeRequirements",
    ),
    (
        SIDECAR_START_FAILED,
        "dub.error.sidecarStart",
        "dub.next.reinstallRuntime",
    ),
    (
        SIDECAR_CRASHED,
        "dub.error.sidecarCrashed",
        "dub.next.retry",
    ),
    (STAGE_FAILED, "dub.error.stageFailed", "dub.next.retry"),
    (
        SOURCE_CHANGED,
        "dub.error.sourceChanged",
        "dub.next.restartRun",
    ),
    (
        NOTHING_TO_RESUME,
        "dub.error.nothingToResume",
        "dub.next.startRun",
    ),
    (
        FILE_EXISTS,
        "dub.error.fileExists",
        "dub.next.chooseAnotherName",
    ),
    (
        RUN_NOT_COMPLETE,
        "dub.error.runNotComplete",
        "dub.next.finishRun",
    ),
    (CANCELLED, "dub.error.cancelled", "dub.next.resume"),
    (
        PATH_NOT_ALLOWED,
        "dub.error.pathNotAllowed",
        "dub.next.retry",
    ),
    (
        RUNTIME_MISSING,
        "dub.error.runtimeMissing",
        "dub.next.downloadModels",
    ),
    (
        NOT_IMPLEMENTED,
        "dub.error.notImplemented",
        "dub.next.waitForUpdate",
    ),
    (
        INVALID_PROJECT,
        "error.invalidProject",
        "dub.next.chooseAnotherFile",
    ),
];

pub fn next_step_key(code: &str) -> &'static str {
    DUB_ERRORS
        .iter()
        .find(|(c, _, _)| *c == code)
        .map(|(_, _, n)| *n)
        .unwrap_or("dub.next.retry")
}
