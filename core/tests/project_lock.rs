use flickfluent_core::error::READ_ONLY;
use flickfluent_core::project::ProjectSession;

fn lock_path(path: &std::path::Path) -> std::path::PathBuf {
    let mut s = path.as_os_str().to_owned();
    s.push(".lock");
    s.into()
}

#[test]
fn second_opener_is_read_only_and_cannot_save() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("p.flick");
    let first = ProjectSession::create("Locked", &path).unwrap();
    assert!(!first.read_only);

    let mut second = ProjectSession::open(&path).unwrap().session;
    assert!(second.read_only);
    second.set_data(serde_json::json!({ "x": 1 }));
    assert_eq!(second.save().err().unwrap().code, READ_ONLY);

    first.close();
    let third = ProjectSession::open(&path).unwrap().session;
    assert!(
        !third.read_only,
        "lock is released when the first session closes"
    );
}

#[test]
fn a_read_only_session_does_not_remove_the_other_owners_lock() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("p.flick");
    let first = ProjectSession::create("Locked", &path).unwrap();
    let second = ProjectSession::open(&path).unwrap().session;
    second.close();
    assert!(lock_path(&path).exists());
    drop(first);
    assert!(!lock_path(&path).exists());
}

#[test]
fn a_stale_lock_from_a_dead_process_is_ignored() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("p.flick");
    ProjectSession::create("Stale", &path).unwrap().close();
    // A process id that cannot be running.
    std::fs::write(lock_path(&path), "2000000000").unwrap();

    let s = ProjectSession::open(&path).unwrap().session;
    assert!(!s.read_only);
}

#[test]
fn a_garbled_lock_file_is_treated_as_stale() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("p.flick");
    ProjectSession::create("Garbled", &path).unwrap().close();
    std::fs::write(lock_path(&path), "not a number").unwrap();
    assert!(!ProjectSession::open(&path).unwrap().session.read_only);
}
