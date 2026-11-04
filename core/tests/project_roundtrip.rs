use flickfluent_core::project::{file_name_for, ProjectSession};
use serde_json::json;

#[test]
fn save_and_reopen_keeps_everything_with_non_latin_names() {
    let dir = tempfile::tempdir().unwrap();
    for name in ["សាកល្បង Project", "项目 测试", "Ünïcode ✓"] {
        let path = dir.path().join(file_name_for(name));
        let mut s = ProjectSession::create(name, &path).unwrap();
        let id = s.project.manifest.id;
        s.set_data(json!({ "notes": "សួស្តី", "n": 3 }));
        s.save().unwrap();
        s.close();

        let reopened = ProjectSession::open(&path).unwrap().session;
        assert!(!reopened.read_only);
        assert!(!reopened.dirty);
        assert_eq!(reopened.project.manifest.name, name);
        assert_eq!(reopened.project.manifest.id, id);
        assert_eq!(reopened.project.data, json!({ "notes": "សួស្តី", "n": 3 }));
        assert!(reopened.project.manifest.saved_at >= reopened.project.manifest.created_at);
    }
}

#[test]
fn file_names_are_safe_on_both_platforms() {
    assert_eq!(file_name_for("a/b:c*d?"), "a_b_c_d_.flick");
    assert_eq!(file_name_for("  "), "Untitled.flick");
    assert_eq!(file_name_for("My film.flick"), "My film.flick");
    assert_eq!(file_name_for("ភាពយន្ត"), "ភាពយន្ត.flick");
}

#[test]
fn save_as_moves_the_session_to_the_new_file() {
    let dir = tempfile::tempdir().unwrap();
    let first = dir.path().join("one.flick");
    let second = dir.path().join("two.flick");
    let mut s = ProjectSession::create("One", &first).unwrap();
    s.set_data(json!({ "k": 1 }));
    s.save_as(&second).unwrap();
    assert_eq!(s.path, second);
    s.close();

    assert_eq!(
        ProjectSession::open(&second).unwrap().session.project.data,
        json!({ "k": 1 })
    );
    // The first file keeps what it had when the project was created.
    assert_eq!(
        ProjectSession::open(&first).unwrap().session.project.data,
        json!({})
    );
}

#[test]
fn a_new_project_file_exists_straight_away() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("new.flick");
    let s = ProjectSession::create("New", &path).unwrap();
    assert!(path.exists());
    assert!(!s.dirty);
}
