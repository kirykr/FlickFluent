mod support;

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use flickfluent_core::error::{HASH_MISMATCH, NO_SPACE, OFFLINE};
use flickfluent_core::models::{download, part_path, Control, Outcome, Phase, Progress};
use support::{body, dead_url, entry, serve, Behavior};

async fn run(
    e: &flickfluent_core::models::CatalogueEntry,
    dir: &std::path::Path,
    control: &Control,
) -> Result<Outcome, flickfluent_core::error::AppError> {
    download(e, dir, control, &mut |_| {}).await
}

#[tokio::test]
async fn downloads_verifies_and_installs() {
    let data = body(200_000);
    let server = serve(Behavior::new(data.clone()));
    let dir = tempfile::tempdir().unwrap();
    let e = entry(&server.url, &data);

    let outcome = run(&e, dir.path(), &Control::new()).await.unwrap();
    let Outcome::Completed(path) = outcome else {
        panic!("expected completed")
    };
    assert_eq!(std::fs::read(&path).unwrap(), data);
    assert!(
        !part_path(dir.path(), &e).exists(),
        "the .part file is gone"
    );
}

#[tokio::test]
async fn progress_reports_bytes_then_a_verifying_phase() {
    let data = body(100_000);
    let server = serve(Behavior::new(data.clone()));
    let dir = tempfile::tempdir().unwrap();
    let e = entry(&server.url, &data);

    let mut seen: Vec<Progress> = Vec::new();
    download(&e, dir.path(), &Control::new(), &mut |p| seen.push(p))
        .await
        .unwrap();
    assert!(seen
        .iter()
        .any(|p| p.phase == Phase::Downloading && p.bytes_done > 0));
    assert_eq!(seen.last().unwrap().phase, Phase::Verifying);
    assert!(seen.windows(2).all(|w| w[0].bytes_done <= w[1].bytes_done));
}

#[tokio::test]
async fn resumes_after_the_connection_drops() {
    let data = body(200_000);
    let mut b = Behavior::new(data.clone());
    b.cut_first_after = Some(40_000);
    let server = serve(b);
    let dir = tempfile::tempdir().unwrap();
    let e = entry(&server.url, &data);

    let err = run(&e, dir.path(), &Control::new()).await.unwrap_err();
    assert_eq!(err.code, OFFLINE);
    let kept = std::fs::metadata(part_path(dir.path(), &e)).unwrap().len();
    assert!(kept > 0 && kept < 200_000, "progress is kept: {kept}");

    let Outcome::Completed(path) = run(&e, dir.path(), &Control::new()).await.unwrap() else {
        panic!("expected completed")
    };
    assert_eq!(std::fs::read(path).unwrap(), data);
    let ranges = server.requests.lock().unwrap().clone();
    assert_eq!(ranges.len(), 2);
    assert_eq!(
        ranges[1],
        Some(format!("{kept}-")),
        "second request resumed from the kept bytes"
    );
}

#[tokio::test]
async fn pause_keeps_the_part_file_and_resume_finishes() {
    let data = body(300_000);
    let mut b = Behavior::new(data.clone());
    b.chunk_delay_ms = 2;
    let server = serve(b);
    let dir = tempfile::tempdir().unwrap();
    let e = entry(&server.url, &data);

    let control = Control::new();
    let c = control.clone();
    let done = Arc::new(AtomicU64::new(0));
    let d = done.clone();
    let outcome = download(&e, dir.path(), &control, &mut move |p| {
        d.store(p.bytes_done, Ordering::SeqCst);
        if p.bytes_done >= 50_000 {
            c.pause();
        }
    })
    .await
    .unwrap();
    assert_eq!(outcome, Outcome::Paused);
    let kept = std::fs::metadata(part_path(dir.path(), &e)).unwrap().len();
    assert!((50_000..300_000).contains(&kept), "kept {kept}");

    let Outcome::Completed(path) = run(&e, dir.path(), &Control::new()).await.unwrap() else {
        panic!("expected completed")
    };
    assert_eq!(std::fs::read(path).unwrap(), data);
}

#[tokio::test]
async fn cancel_removes_the_part_file() {
    let data = body(300_000);
    let mut b = Behavior::new(data.clone());
    b.chunk_delay_ms = 2;
    let server = serve(b);
    let dir = tempfile::tempdir().unwrap();
    let e = entry(&server.url, &data);

    let control = Control::new();
    let c = control.clone();
    let outcome = download(&e, dir.path(), &control, &mut move |p| {
        if p.bytes_done >= 30_000 {
            c.cancel();
        }
    })
    .await
    .unwrap();
    assert_eq!(outcome, Outcome::Cancelled);
    assert!(!part_path(dir.path(), &e).exists());
}

#[tokio::test]
async fn a_wrong_hash_is_never_installed() {
    let data = body(50_000);
    let server = serve(Behavior::new(data.clone()));
    let dir = tempfile::tempdir().unwrap();
    let mut e = entry(&server.url, &data);
    e.sha256 = "0".repeat(64);

    let err = run(&e, dir.path(), &Control::new()).await.unwrap_err();
    assert_eq!(err.code, HASH_MISMATCH);
    assert!(
        !part_path(dir.path(), &e).exists(),
        "the bad file is deleted"
    );
    let leftovers: Vec<_> = walk(dir.path())
        .into_iter()
        .filter(|p| p.is_file())
        .collect();
    assert!(leftovers.is_empty(), "{leftovers:?}");
}

#[tokio::test]
async fn refuses_when_there_is_not_enough_disk_space() {
    let data = body(1_000);
    let server = serve(Behavior::new(data.clone()));
    let dir = tempfile::tempdir().unwrap();
    let mut e = entry(&server.url, &data);
    e.size_bytes = u64::MAX / 4; // far more than any disk

    let err = run(&e, dir.path(), &Control::new()).await.unwrap_err();
    assert_eq!(err.code, NO_SPACE);
    let d = err.details.unwrap();
    assert_eq!(d["neededBytes"], u64::MAX / 4);
    assert!(d["freeBytes"].as_u64().unwrap() > 0);
    assert!(
        server.requests.lock().unwrap().is_empty(),
        "no download was started"
    );
}

#[tokio::test]
async fn reports_offline_when_the_server_cannot_be_reached() {
    let data = body(1_000);
    let dir = tempfile::tempdir().unwrap();
    let e = entry(&dead_url(), &data);
    let err = run(&e, dir.path(), &Control::new()).await.unwrap_err();
    assert_eq!(err.code, OFFLINE);
}

#[tokio::test]
async fn a_server_that_ignores_range_restarts_from_zero() {
    let data = body(120_000);
    let mut b = Behavior::new(data.clone());
    b.honour_range = false;
    b.cut_first_after = Some(30_000);
    let server = serve(b);
    let dir = tempfile::tempdir().unwrap();
    let e = entry(&server.url, &data);

    assert_eq!(
        run(&e, dir.path(), &Control::new()).await.unwrap_err().code,
        OFFLINE
    );
    let Outcome::Completed(path) = run(&e, dir.path(), &Control::new()).await.unwrap() else {
        panic!("expected completed")
    };
    assert_eq!(
        std::fs::read(path).unwrap(),
        data,
        "no duplicated or missing bytes"
    );
}

#[tokio::test]
async fn a_leftover_part_file_that_is_too_big_is_discarded() {
    let data = body(10_000);
    let server = serve(Behavior::new(data.clone()));
    let dir = tempfile::tempdir().unwrap();
    let e = entry(&server.url, &data);
    let part = part_path(dir.path(), &e);
    std::fs::create_dir_all(part.parent().unwrap()).unwrap();
    std::fs::write(&part, vec![7u8; 50_000]).unwrap();

    let Outcome::Completed(path) = run(&e, dir.path(), &Control::new()).await.unwrap() else {
        panic!("expected completed")
    };
    assert_eq!(std::fs::read(path).unwrap(), data);
}

fn walk(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut out = vec![];
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        let p = e.path();
        if p.is_dir() {
            out.extend(walk(&p));
        } else {
            out.push(p);
        }
    }
    out
}
