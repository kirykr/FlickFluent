use flickfluent_core::error::{DAMAGED, IO_ERROR};
use flickfluent_core::project::{Project, ProjectSession};
use serde_json::json;

fn sidecar(path: &std::path::Path, suffix: &str) -> std::path::PathBuf {
    let mut s = path.as_os_str().to_owned();
    s.push(suffix);
    s.into()
}

fn save_version(path: &std::path::Path, n: u32) {
    let mut s = if path.exists() {
        ProjectSession::open(path).unwrap().session
    } else {
        ProjectSession::create("Versions", path).unwrap()
    };
    s.set_data(json!({ "v": n }));
    s.save().unwrap();
    s.close();
}

#[test]
fn damaged_file_reports_backup_and_recovers_the_last_good_version() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("p.flick");
    save_version(&path, 1);
    save_version(&path, 2);

    std::fs::write(&path, b"corrupted").unwrap();
    let err = ProjectSession::open(&path).err().unwrap();
    assert_eq!(err.code, DAMAGED);
    assert_eq!(
        err.details.as_ref().unwrap()["backupAvailable"],
        json!(true)
    );

    let recovered = ProjectSession::open_recovered(&path).unwrap();
    assert_eq!(recovered.project.data, json!({ "v": 1 }));
    assert!(recovered.dirty, "recovered work must be saved by the user");
}

#[test]
fn damaged_file_without_any_backup_says_so() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("lonely.flick");
    std::fs::write(&path, b"corrupted").unwrap();
    let err = ProjectSession::open(&path).err().unwrap();
    assert_eq!(
        err.details.as_ref().unwrap()["backupAvailable"],
        json!(false)
    );
}

#[test]
fn only_the_last_three_backups_are_kept() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("p.flick");
    for n in 1..=6 {
        save_version(&path, n);
    }
    let data = |p: &std::path::Path| Project::read(p).unwrap().data;
    assert_eq!(data(&path), json!({ "v": 6 }));
    assert_eq!(data(&sidecar(&path, ".bak.1")), json!({ "v": 5 }));
    assert_eq!(data(&sidecar(&path, ".bak.2")), json!({ "v": 4 }));
    assert_eq!(data(&sidecar(&path, ".bak.3")), json!({ "v": 3 }));
    assert!(!sidecar(&path, ".bak.4").exists());
}

#[test]
fn a_damaged_file_is_not_copied_over_a_good_backup() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("p.flick");
    save_version(&path, 1);
    save_version(&path, 2);
    std::fs::write(&path, b"corrupted").unwrap();

    let mut s = ProjectSession::open_recovered(&path).unwrap();
    s.save().unwrap();
    s.close();
    // bak.1 must still hold a readable project, never the corrupted bytes.
    assert!(Project::read(&sidecar(&path, ".bak.1")).is_ok());
}

#[test]
fn save_leaves_no_temporary_file_behind() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("p.flick");
    save_version(&path, 1);
    let names: Vec<String> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert!(names.iter().all(|n| !n.ends_with(".tmp")), "{names:?}");
}

#[test]
fn a_failed_write_never_creates_a_half_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("missing-folder").join("p.flick");
    let err = ProjectSession::create("X", &path).err().unwrap();
    assert_eq!(err.code, IO_ERROR);
    assert!(!path.exists());
}

#[test]
fn crash_leaves_a_recovery_file_and_it_is_offered_on_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("p.flick");
    save_version(&path, 1);

    let mut s = ProjectSession::open(&path).unwrap().session;
    s.set_data(json!({ "v": 2, "unsaved": true }));
    assert!(s.autosave_tick().unwrap());
    drop(s); // simulates a crash: no save and no close

    assert!(sidecar(&path, ".recover").exists());
    let outcome = ProjectSession::open(&path).unwrap();
    assert!(outcome.recovery_available);
    assert_eq!(
        outcome.session.project.data,
        json!({ "v": 1 }),
        "the saved file is untouched"
    );
    outcome.session.close();

    let recovered = ProjectSession::open_recovered(&path).unwrap();
    assert_eq!(recovered.project.data, json!({ "v": 2, "unsaved": true }));
    assert!(recovered.dirty);
}

#[test]
fn autosave_does_nothing_when_nothing_changed() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("p.flick");
    let mut s = ProjectSession::create("Clean", &path).unwrap();
    assert!(!s.autosave_tick().unwrap());
    assert!(!sidecar(&path, ".recover").exists());
}

#[test]
fn save_and_close_remove_the_recovery_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("p.flick");
    let mut s = ProjectSession::create("R", &path).unwrap();
    s.set_data(json!({ "a": 1 }));
    s.autosave_tick().unwrap();
    assert!(sidecar(&path, ".recover").exists());
    s.save().unwrap();
    assert!(!sidecar(&path, ".recover").exists());

    s.set_data(json!({ "a": 2 }));
    s.autosave_tick().unwrap();
    s.close();
    assert!(!sidecar(&path, ".recover").exists());
}
