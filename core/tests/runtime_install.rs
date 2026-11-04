//! Installing the runtime pack: download, verify, unpack, then start the sidecar and say hello.
//! The pack is a fake: a `python3` shell script that runs the system Python, so no real pack is
//! downloaded. Unix only because of that script.
#![cfg(unix)]

mod support;

use std::io::Write;
use std::path::Path;
use std::time::Duration;

use flickfluent_core::error::{HASH_MISMATCH, SIDECAR_START_FAILED};
use flickfluent_core::models::runtime::{install, python_exe, runtime_dir};
use flickfluent_core::models::{Control, Outcome};
use support::{entry, serve, Behavior};

fn tar_gz(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut raw = Vec::new();
    {
        let mut b = tar::Builder::new(&mut raw);
        for (name, data) in entries {
            let mut h = tar::Header::new_gnu();
            h.set_size(data.len() as u64);
            h.set_mode(0o755);
            h.set_cksum();
            b.append_data(&mut h, name, *data).unwrap();
        }
        b.finish().unwrap();
    }
    let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    gz.write_all(&raw).unwrap();
    gz.finish().unwrap()
}

fn fake_pack(interpreter: &str) -> Vec<u8> {
    let script = format!("#!/bin/sh\nexec {interpreter} \"$@\"\n");
    tar_gz(&[("python/bin/python3", script.as_bytes())])
}

fn stub_args() -> Vec<std::ffi::OsString> {
    let stub = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/support/stub_sidecar.py");
    vec![
        stub.into_os_string(),
        "--allow".into(),
        std::env::temp_dir().into_os_string(),
    ]
}

async fn run(
    body: Vec<u8>,
    models: &Path,
    health_args: Vec<std::ffi::OsString>,
) -> Result<Outcome, flickfluent_core::error::AppError> {
    let server = serve(Behavior::new(body.clone()));
    let mut e = entry(&server.url, &body);
    e.id = "runtime-test".into();
    e.file_name = Some("pack.tar.gz".into());
    install(
        &e,
        models,
        &Control::new(),
        &mut |_| {},
        None,
        health_args,
        Duration::from_secs(20),
    )
    .await
}

#[tokio::test]
async fn installs_unpacks_and_the_sidecar_answers_hello() {
    let dir = tempfile::tempdir().unwrap();
    let outcome = run(fake_pack("python3"), dir.path(), stub_args())
        .await
        .unwrap();
    let Outcome::Completed(root) = outcome else {
        panic!("expected Completed")
    };
    assert_eq!(root, runtime_dir(dir.path()));
    assert!(python_exe(dir.path()).is_ok());
    // The archive is removed once unpacked, to save disk.
    assert!(!dir.path().join("runtime-test").join("pack.tar.gz").exists());
}

#[tokio::test]
async fn a_failed_health_check_removes_the_runtime() {
    let dir = tempfile::tempdir().unwrap();
    let err = run(fake_pack("false"), dir.path(), stub_args())
        .await
        .unwrap_err();
    assert_eq!(err.code, SIDECAR_START_FAILED);
    assert!(!runtime_dir(dir.path()).exists());
}

#[tokio::test]
async fn a_wrong_hash_installs_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let body = fake_pack("python3");
    let server = serve(Behavior::new(body.clone()));
    let mut e = entry(&server.url, &body);
    e.sha256 = "0".repeat(64);
    let err = install(
        &e,
        dir.path(),
        &Control::new(),
        &mut |_| {},
        None,
        stub_args(),
        Duration::from_secs(5),
    )
    .await
    .unwrap_err();
    assert_eq!(err.code, HASH_MISMATCH);
    assert!(!runtime_dir(dir.path()).exists());
}

/// A fake interpreter that records every call and fails `-m pip` when told to.
fn recording_pack(log: &Path, pip_ok: bool) -> Vec<u8> {
    let script = format!(
        "#!/bin/sh\necho \"$@\" >> {log}\nif [ \"$1\" = \"-m\" ] && [ \"$2\" = \"pip\" ]; then exit {code}; fi\nexec python3 \"$@\"\n",
        log = log.display(),
        code = if pip_ok { 0 } else { 1 }
    );
    tar_gz(&[("python/bin/python3", script.as_bytes())])
}

#[tokio::test]
async fn packages_are_installed_with_hashes_and_wheels_only() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("calls.log");
    let body = recording_pack(&log, true);
    let server = serve(Behavior::new(body.clone()));
    let e = entry(&server.url, &body);
    install(
        &e,
        dir.path(),
        &Control::new(),
        &mut |_| {},
        Some("numpy==1 \\\n    --hash=sha256:00\n"),
        stub_args(),
        Duration::from_secs(20),
    )
    .await
    .unwrap();
    let calls = std::fs::read_to_string(&log).unwrap();
    let pip = calls
        .lines()
        .find(|l| l.starts_with("-m pip"))
        .expect("pip was run");
    for flag in ["--require-hashes", "--only-binary=:all:", "--no-deps", "-r"] {
        assert!(pip.contains(flag), "{pip}");
    }
    assert!(
        std::fs::read_to_string(dir.path().join("runtime/requirements.lock"))
            .unwrap()
            .contains("numpy==1")
    );
}

#[tokio::test]
async fn a_failed_package_install_removes_the_runtime() {
    let dir = tempfile::tempdir().unwrap();
    let body = recording_pack(&dir.path().join("calls.log"), false);
    let server = serve(Behavior::new(body.clone()));
    let e = entry(&server.url, &body);
    let err = install(
        &e,
        dir.path(),
        &Control::new(),
        &mut |_| {},
        Some("x==1 --hash=sha256:00"),
        stub_args(),
        Duration::from_secs(20),
    )
    .await
    .unwrap_err();
    assert_eq!(err.code, flickfluent_core::error::RUNTIME_MISSING);
    assert!(!runtime_dir(dir.path()).exists());
}

#[test]
fn locks_exist_where_the_engine_can_run() {
    use flickfluent_core::models::runtime::lock_for;
    for (os, arch) in [("macos", "arm64"), ("macos", "x64"), ("windows", "x64")] {
        let lock = lock_for(os, arch).unwrap_or_else(|| panic!("{os} {arch}"));
        assert!(lock.contains("--hash=sha256:") && lock.contains("faster-whisper=="));
    }
    assert!(
        lock_for("windows", "arm64").is_none(),
        "no ctranslate2 wheel for Windows on ARM"
    );
    assert!(lock_for("macos", "arm64").unwrap().contains("demucs=="));
    assert!(
        !lock_for("macos", "x64").unwrap().contains("demucs=="),
        "no sphn wheel for Intel Macs"
    );
}
