//! On-demand model downloads: a catalogue, a resumable downloader, integrity checks and storage.

mod catalogue;
mod downloader;
pub mod install;
pub mod runtime;
mod storage;
mod verify;

pub use catalogue::{
    bundled as bundled_catalogue, CatalogueEntry, FileSpec, Gender, LicenceClass, MachineClass,
    Platforms, Quality, Step, VoiceSpec,
};
pub use downloader::{download, Control, Outcome, Phase, Progress};
pub use install::{install_entry, unpack_zip, Target};
pub use storage::{
    delete_model, final_path, free_space, installed, part_bytes, part_path, preflight_space,
    record_installed, InstalledModel,
};
pub use verify::{sha256_file, verify_file};
