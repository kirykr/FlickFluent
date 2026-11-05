#![allow(dead_code)] // each test file uses a different part of this module
//! A tiny HTTP server for download tests. It can honour or ignore `Range`, cut the first
//! response short (to simulate a dropped connection), and send slowly (to leave time to pause).

use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use flickfluent_core::models::CatalogueEntry;
use sha2::{Digest, Sha256};

pub struct Behavior {
    pub body: Vec<u8>,
    pub honour_range: bool,
    /// Send only this many body bytes on the first request, then drop the connection.
    pub cut_first_after: Option<usize>,
    pub chunk_delay_ms: u64,
}

impl Behavior {
    pub fn new(body: Vec<u8>) -> Self {
        Self {
            body,
            honour_range: true,
            cut_first_after: None,
            chunk_delay_ms: 0,
        }
    }
}

pub struct Server {
    pub url: String,
    /// The `Range` header of each request, in order (`None` when absent).
    pub requests: Arc<Mutex<Vec<Option<String>>>>,
}

pub fn serve(b: Behavior) -> Server {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/model.bin", listener.local_addr().unwrap());
    let requests = Arc::new(Mutex::new(Vec::new()));
    let log = requests.clone();
    let cut_available = Arc::new(AtomicBool::new(b.cut_first_after.is_some()));
    let b = Arc::new(b);
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let (b, log, cut) = (b.clone(), log.clone(), cut_available.clone());
            std::thread::spawn(move || handle(stream, &b, &log, &cut));
        }
    });
    Server { url, requests }
}

fn handle(mut stream: TcpStream, b: &Behavior, log: &Mutex<Vec<Option<String>>>, cut: &AtomicBool) {
    let mut buf = Vec::new();
    let mut tmp = [0u8; 1024];
    while !buf.windows(4).any(|w| w == b"\r\n\r\n") {
        match stream.read(&mut tmp) {
            Ok(0) | Err(_) => return,
            Ok(n) => buf.extend_from_slice(&tmp[..n]),
        }
    }
    let head = String::from_utf8_lossy(&buf).to_string();
    let range = head
        .lines()
        .find_map(|l| {
            l.to_ascii_lowercase()
                .strip_prefix("range: bytes=")
                .map(str::to_string)
        })
        .map(|r| r.trim().to_string());
    log.lock().unwrap().push(range.clone());

    let total = b.body.len();
    let start = match (&range, b.honour_range) {
        (Some(r), true) => r.trim_end_matches('-').parse::<usize>().unwrap_or(0),
        _ => 0,
    };
    if start >= total && range.is_some() && b.honour_range {
        let _ = stream.write_all(
            format!("HTTP/1.1 416 Range Not Satisfiable\r\nContent-Range: bytes */{total}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").as_bytes(),
        );
        return;
    }
    let partial = start > 0;
    let body = &b.body[start..];
    let header = if partial {
        format!(
            "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes {start}-{}/{total}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            total - 1,
            body.len()
        )
    } else {
        format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        )
    };
    if stream.write_all(header.as_bytes()).is_err() {
        return;
    }
    let limit = match b.cut_first_after {
        Some(n) if cut.swap(false, Ordering::SeqCst) => n.min(body.len()),
        _ => body.len(),
    };
    for chunk in body[..limit].chunks(8 * 1024) {
        if stream.write_all(chunk).is_err() {
            return;
        }
        if b.chunk_delay_ms > 0 {
            std::thread::sleep(Duration::from_millis(b.chunk_delay_ms));
        }
    }
    let _ = stream.flush();
    let _ = stream.shutdown(Shutdown::Both);
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

pub fn body(len: usize) -> Vec<u8> {
    (0..len).map(|i| (i * 31 % 251) as u8).collect()
}

pub fn entry(url: &str, body: &[u8]) -> CatalogueEntry {
    CatalogueEntry {
        id: "test-model".into(),
        name: "Test model".into(),
        purpose: "Used by tests".into(),
        licence: "MIT".into(),
        size_bytes: body.len() as u64,
        url: url.to_string(),
        sha256: sha256_hex(body),
        ..Default::default()
    }
}

/// A URL where nothing is listening: connecting fails straight away.
pub fn dead_url() -> String {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/model.bin", l.local_addr().unwrap());
    drop(l);
    url
}

/// Helpers for pipeline run tests: a fake model catalogue, models folder and sidecar.
pub mod run_fixture {
    use std::path::{Path, PathBuf};
    use std::time::Duration;

    use flickfluent_core::models::{CatalogueEntry, Gender, MachineClass, Step};
    use flickfluent_core::pipeline::ffmpeg::Ffmpeg;
    use flickfluent_core::pipeline::run::RunConfig;
    use flickfluent_core::pipeline::stages::{fingerprint, probe_source};
    use flickfluent_core::project::dub::{DubData, Options, Source};
    use flickfluent_core::sidecar::SidecarConfig;
    use serde_json::json;

    pub fn ffmpeg() -> Ffmpeg {
        Ffmpeg::locate(Path::new("/nonexistent")).expect("ffmpeg on PATH or FLICK_FFMPEG_DIR")
    }

    pub fn python() -> String {
        std::env::var("FLICK_TEST_PYTHON").unwrap_or_else(|_| {
            if cfg!(windows) {
                "python".into()
            } else {
                "python3".into()
            }
        })
    }

    fn entry(id: &str, step: Step, languages: &[&str], gender: Option<Gender>) -> CatalogueEntry {
        CatalogueEntry {
            id: id.into(),
            step,
            machine_class: MachineClass::Minimum,
            languages: languages.iter().map(|s| s.to_string()).collect(),
            gender,
            default_enabled: true,
            ..Default::default()
        }
    }

    pub fn catalogue() -> Vec<CatalogueEntry> {
        vec![
            entry("whisper", Step::Transcribe, &["fr", "en"], None),
            entry("m2m", Step::Translate, &["fr", "en"], None),
            entry("demucs", Step::Separate, &[], None),
            entry("diar", Step::Speaker, &[], None),
            entry("voice-en-f", Step::Speak, &["en"], Some(Gender::Female)),
            entry("voice-en-m", Step::Speak, &["en"], Some(Gender::Male)),
        ]
    }

    /// A models folder with an empty folder per catalogue entry, and a run config that starts the
    /// fake sidecar. `params` go to every worker.
    pub fn config(root: &Path, project: &str, params: serde_json::Value) -> RunConfig {
        let models = root.join("models");
        for e in catalogue() {
            std::fs::create_dir_all(models.join(&e.id)).unwrap();
        }
        let script =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/support/fake_models_sidecar.py");
        RunConfig {
            cloud: None,
            project_path: root.join(project),
            project_id: "p1".into(),
            models_dir: models,
            catalogue: catalogue(),
            class: MachineClass::Minimum,
            os: if cfg!(windows) {
                "windows".into()
            } else {
                "macos".into()
            },
            arch: "arm64".into(),
            ffmpeg: ffmpeg(),
            sidecar: SidecarConfig {
                program: PathBuf::from(python()),
                args: vec![
                    script.into_os_string(),
                    "--allow".into(),
                    root.as_os_str().to_owned(),
                ],
                hang_timeout: Duration::from_secs(60),
                start_timeout: Duration::from_secs(20),
            },
            code_map: json!({}),
            params,
        }
    }

    /// Project data for a source file, as `dub_configure` would produce it.
    pub async fn dub_for(source: &Path, target: &str) -> DubData {
        let info = probe_source(&ffmpeg(), source).await.unwrap();
        DubData {
            source: Some(Source {
                path: source.to_string_lossy().into_owned(),
                duration_ms: info.duration_ms,
                has_audio: info.has_audio,
                has_video: info.has_video,
                size_bytes: info.size_bytes,
                fingerprint: fingerprint(source).unwrap(),
            }),
            source_language: "fr".into(),
            target_language: target.into(),
            options: Options::default(),
            ..Default::default()
        }
    }

    /// A video of about `minutes` minutes made by looping the 2-minute fixture. Several sections.
    pub fn long_clip(dir: &Path, minutes: u32) -> PathBuf {
        let out = dir.join("long.mp4");
        let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../fixtures/clip-2min-fr.mp4");
        let loops = (minutes / 2).max(1) - 1;
        let status = std::process::Command::new("ffmpeg")
            .args([
                "-v",
                "error",
                "-y",
                "-stream_loop",
                &loops.to_string(),
                "-i",
            ])
            .arg(&src)
            .args(["-c", "copy"])
            .arg(&out)
            .status()
            .unwrap();
        assert!(status.success());
        out
    }
}

pub mod cloud_run;
pub mod fake_cloud;
