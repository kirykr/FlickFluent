//! The core talks to a sidecar over JSON lines: handshake, progress, cancel, crash policy.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use flickfluent_core::sidecar::{Event, Request, Sidecar, SidecarConfig};
use serde_json::json;

fn python() -> String {
    std::env::var("FLICK_TEST_PYTHON").unwrap_or_else(|_| {
        if cfg!(windows) {
            "python".into()
        } else {
            "python3".into()
        }
    })
}

fn config(dir: &Path) -> SidecarConfig {
    let stub = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/support/stub_sidecar.py");
    SidecarConfig {
        program: PathBuf::from(python()),
        args: vec![
            stub.into_os_string(),
            "--allow".into(),
            dir.as_os_str().to_owned(),
        ],
        hang_timeout: Duration::from_secs(2),
        start_timeout: Duration::from_secs(20),
    }
}

fn request(worker: &str, dir: &Path, params: serde_json::Value) -> Request {
    Request {
        worker: worker.into(),
        input: json!({ "audio": dir.join("a.wav") }),
        output: json!({ "dir": dir, "prefix": "sec0001" }),
        models: json!({}),
        params,
    }
}

#[tokio::test]
async fn handshake_progress_and_result() {
    let dir = tempfile::tempdir().unwrap();
    let mut sc = Sidecar::start(config(dir.path())).await.unwrap();
    assert!(sc.workers().contains(&"work".to_string()));

    let progress = Arc::new(AtomicU32::new(0));
    let seen = progress.clone();
    let done = sc
        .run(
            request("work", dir.path(), json!({ "steps": 3 })),
            &mut move |e| {
                if let Event::Progress(_) = e {
                    seen.fetch_add(1, Ordering::SeqCst);
                }
            },
        )
        .await
        .unwrap();
    assert_eq!(done.files, vec!["sec0001.json".to_string()]);
    assert!(progress.load(Ordering::SeqCst) >= 3);
    assert!(dir.path().join("sec0001.json").exists());
    assert!(!dir.path().join("sec0001.json.part").exists());
    sc.shutdown().await;
}

#[tokio::test]
async fn worker_error_keeps_its_code() {
    let dir = tempfile::tempdir().unwrap();
    let mut sc = Sidecar::start(config(dir.path())).await.unwrap();
    let err = sc
        .run(request("fail", dir.path(), json!({})), &mut |_| {})
        .await
        .unwrap_err();
    assert_eq!(err.code, "STAGE_FAILED");
    sc.shutdown().await;
}

#[tokio::test]
async fn path_outside_allowed_folders_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let other = tempfile::tempdir().unwrap();
    let mut sc = Sidecar::start(config(dir.path())).await.unwrap();
    let err = sc
        .run(request("work", other.path(), json!({})), &mut |_| {})
        .await
        .unwrap_err();
    assert_eq!(err.code, "PATH_NOT_ALLOWED");
    sc.shutdown().await;
}

#[tokio::test]
async fn cancel_answers_cancelled_within_five_seconds() {
    let dir = tempfile::tempdir().unwrap();
    let mut sc = Sidecar::start(config(dir.path())).await.unwrap();
    let cancel = sc.cancel_handle();
    let started = Instant::now();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(300)).await;
        cancel.cancel().await;
    });
    let err = sc
        .run(
            request(
                "work",
                dir.path(),
                json!({ "steps": 100, "stepSeconds": 0.1 }),
            ),
            &mut |_| {},
        )
        .await
        .unwrap_err();
    assert_eq!(err.code, "CANCELLED");
    assert!(started.elapsed() < Duration::from_secs(5));
    assert!(!dir.path().join("sec0001.json").exists());
    sc.shutdown().await;
}

#[tokio::test]
async fn a_crash_restarts_once_and_retries() {
    let dir = tempfile::tempdir().unwrap();
    let marker = dir.path().join("crashed.marker");
    let mut sc = Sidecar::start(config(dir.path())).await.unwrap();
    let done = sc
        .run_with_retry(
            request("crash_once", dir.path(), json!({ "marker": marker })),
            &mut |_| {},
        )
        .await
        .unwrap();
    assert!(done.ok);
    assert!(marker.exists(), "first attempt really crashed");
    sc.shutdown().await;
}

#[tokio::test]
async fn a_second_crash_gives_sidecar_crashed() {
    let dir = tempfile::tempdir().unwrap();
    let mut sc = Sidecar::start(config(dir.path())).await.unwrap();
    let err = sc
        .run_with_retry(request("always_crash", dir.path(), json!({})), &mut |_| {})
        .await
        .unwrap_err();
    assert_eq!(err.code, "SIDECAR_CRASHED");
    sc.shutdown().await;
}

#[tokio::test]
async fn silence_past_the_timeout_counts_as_a_hang() {
    let dir = tempfile::tempdir().unwrap();
    let mut cfg = config(dir.path());
    cfg.hang_timeout = Duration::from_millis(700);
    let mut sc = Sidecar::start(cfg).await.unwrap();
    let started = Instant::now();
    let err = sc
        .run_with_retry(request("hang", dir.path(), json!({})), &mut |_| {})
        .await
        .unwrap_err();
    assert_eq!(err.code, "SIDECAR_CRASHED");
    assert!(started.elapsed() < Duration::from_secs(10));
    sc.shutdown().await;
}

#[tokio::test]
async fn a_missing_program_gives_start_failed() {
    let dir = tempfile::tempdir().unwrap();
    let mut cfg = config(dir.path());
    cfg.program = PathBuf::from("definitely-not-a-program-xyz");
    let err = Sidecar::start(cfg).await.err().expect("must fail");
    assert_eq!(err.code, "SIDECAR_START_FAILED");
}
