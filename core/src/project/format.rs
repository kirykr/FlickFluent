//! The `.flick` file: a ZIP with `manifest.json`, `project.json` and reserved `assets/`.
//! See `specs/001-app-shell-hardware-check/contracts/project-file-format.md`.

use std::collections::BTreeMap;
use std::io::{Cursor, Read, Write};
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use uuid::Uuid;
use zip::write::SimpleFileOptions;

use super::sidecar;
use crate::error::{AppError, DAMAGED, FORMAT_TOO_NEW};

pub const SUPPORTED_FORMAT_VERSION: u32 = 2;
const MANIFEST: &str = "manifest.json";
const PROJECT: &str = "project.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub format_version: u32,
    pub app_version: String,
    pub id: Uuid,
    /// Display name. The file name never has to match it.
    pub name: String,
    pub created_at: String,
    pub saved_at: String,
    /// Keys this version does not know are kept and written back (rule 2 of the contract).
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone)]
pub struct Project {
    pub manifest: Manifest,
    /// Feature data. Empty in feature 001; later features add their own keys.
    pub data: Value,
    /// Every other ZIP entry (for example `assets/...`), kept as is.
    pub entries: BTreeMap<String, Vec<u8>>,
}

pub fn now() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_default()
}

fn damaged() -> AppError {
    AppError::new(DAMAGED, "error.damaged")
}

impl Project {
    pub fn new(name: &str) -> Self {
        let stamp = now();
        Self {
            manifest: Manifest {
                format_version: SUPPORTED_FORMAT_VERSION,
                app_version: env!("CARGO_PKG_VERSION").to_string(),
                id: Uuid::new_v4(),
                name: name.to_string(),
                created_at: stamp.clone(),
                saved_at: stamp,
                extra: Map::new(),
            },
            data: Value::Object(Map::new()),
            entries: BTreeMap::new(),
        }
    }

    /// Read a project. A newer format is refused before anything else is parsed.
    pub fn read(path: &Path) -> Result<Self, AppError> {
        let file = std::fs::File::open(path).map_err(AppError::io)?;
        let mut zip = zip::ZipArchive::new(file).map_err(|_| damaged())?;

        let raw: Value =
            serde_json::from_slice(&read_entry(&mut zip, MANIFEST)?).map_err(|_| damaged())?;
        let version = raw
            .get("formatVersion")
            .and_then(Value::as_u64)
            .ok_or_else(damaged)?;
        if version > u64::from(SUPPORTED_FORMAT_VERSION) {
            return Err(AppError::new(FORMAT_TOO_NEW, "error.formatTooNew")
                .with_details(serde_json::json!({ "fileVersion": version })));
        }
        let mut manifest: Manifest = serde_json::from_value(raw).map_err(|_| damaged())?;
        let mut data: Value =
            serde_json::from_slice(&read_entry(&mut zip, PROJECT)?).map_err(|_| damaged())?;
        if version < 2 {
            migrate_1_to_2(path, &mut manifest, &mut data)?;
        }

        let mut entries = BTreeMap::new();
        for i in 0..zip.len() {
            let mut entry = zip.by_index(i).map_err(|_| damaged())?;
            let name = entry.name().to_string();
            if entry.is_dir() || name == MANIFEST || name == PROJECT {
                continue;
            }
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes).map_err(|_| damaged())?;
            entries.insert(name, bytes);
        }
        Ok(Self {
            manifest,
            data,
            entries,
        })
    }

    /// Write to a temporary file, flush it to disk, then rename over the target. The target is
    /// never left half written.
    pub fn write_atomic(&self, path: &Path) -> Result<(), AppError> {
        let bytes = self.to_zip_bytes()?;
        let tmp = sidecar(path, ".tmp");
        let result = (|| -> std::io::Result<()> {
            let mut file = std::fs::File::create(&tmp)?;
            file.write_all(&bytes)?;
            file.sync_all()?;
            std::fs::rename(&tmp, path)
        })();
        if let Err(e) = result {
            let _ = std::fs::remove_file(&tmp);
            return Err(AppError::io(e));
        }
        Ok(())
    }

    fn to_zip_bytes(&self) -> Result<Vec<u8>, AppError> {
        let opts =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let mut put = |name: &str, bytes: &[u8]| -> Result<(), AppError> {
            zip.start_file(name, opts)
                .map_err(|e| AppError::io(e.into()))?;
            zip.write_all(bytes).map_err(AppError::io)
        };
        put(
            MANIFEST,
            &serde_json::to_vec_pretty(&self.manifest).map_err(|_| damaged())?,
        )?;
        put(
            PROJECT,
            &serde_json::to_vec(&self.data).map_err(|_| damaged())?,
        )?;
        for (name, bytes) in &self.entries {
            put(name, bytes)?;
        }
        Ok(zip
            .finish()
            .map_err(|e| AppError::io(e.into()))?
            .into_inner())
    }
}

/// Format 1 had no dubbing data. Add the empty v2 fields and keep a copy of the old file once.
/// The file itself becomes format 2 on the next save.
fn migrate_1_to_2(path: &Path, manifest: &mut Manifest, data: &mut Value) -> Result<(), AppError> {
    let backup = sidecar(path, ".v1.bak");
    if !backup.exists() {
        std::fs::copy(path, &backup).map_err(AppError::io)?;
    }
    manifest.format_version = 2;
    if let (Value::Object(map), Ok(Value::Object(defaults))) =
        (data, serde_json::to_value(super::dub::DubData::default()))
    {
        for (k, v) in defaults {
            map.entry(k).or_insert(v);
        }
    }
    Ok(())
}

fn read_entry(zip: &mut zip::ZipArchive<std::fs::File>, name: &str) -> Result<Vec<u8>, AppError> {
    let mut entry = zip.by_name(name).map_err(|_| damaged())?;
    let mut bytes = Vec::new();
    entry.read_to_end(&mut bytes).map_err(|_| damaged())?;
    Ok(bytes)
}
