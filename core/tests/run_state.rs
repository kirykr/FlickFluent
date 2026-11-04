//! `run-state.json` is crash safe and drives resume.

use flickfluent_core::pipeline::state::{input_hash, WorkFolder};
use flickfluent_core::project::dub::{PipelineRun, RunState, Section, Stage, StageStatus};

fn run_with_section(hash: &str) -> PipelineRun {
    let mut section = Section {
        index: 0,
        start_ms: 0,
        end_ms: 300_000,
        ..Default::default()
    };
    section.status.insert(Stage::Transcribe, StageStatus::Done);
    section
        .input_hash
        .insert(Stage::Transcribe, hash.to_string());
    PipelineRun {
        id: "r1".into(),
        state: RunState::Running,
        sections: vec![section],
        ..Default::default()
    }
}

#[test]
fn a_torn_write_leaves_the_previous_state_readable() {
    let dir = tempfile::tempdir().unwrap();
    let work = WorkFolder::create(&dir.path().join("film.flick"), "project-1").unwrap();
    work.save_run(&run_with_section("h1")).unwrap();

    // A kill in the middle of the next write leaves a half-written temp file behind.
    std::fs::write(
        work.path().join("run-state.json.tmp"),
        b"{\"id\":\"r2\",\"sta",
    )
    .unwrap();

    let loaded = work
        .load_run()
        .unwrap()
        .expect("previous state is still there");
    assert_eq!(loaded.id, "r1");
}

#[test]
fn a_run_left_running_is_reported_as_interrupted() {
    let dir = tempfile::tempdir().unwrap();
    let work = WorkFolder::create(&dir.path().join("film.flick"), "project-1").unwrap();
    assert!(!work.is_interrupted());
    work.save_run(&run_with_section("h1")).unwrap();
    assert!(work.is_interrupted());

    let mut done = run_with_section("h1");
    done.state = RunState::Completed;
    work.save_run(&done).unwrap();
    assert!(!work.is_interrupted());
}

#[test]
fn stage_is_skipped_only_when_output_exists_parses_and_hash_matches() {
    let dir = tempfile::tempdir().unwrap();
    let work = WorkFolder::create(&dir.path().join("film.flick"), "project-1").unwrap();
    let run = run_with_section("h1");
    let out = work.section_dir(0).join("lines.json");

    // Output missing: not skippable.
    assert!(!work.can_skip(&run, 0, Stage::Transcribe, "h1", &[&out]));

    // Output damaged: not skippable.
    std::fs::create_dir_all(out.parent().unwrap()).unwrap();
    std::fs::write(&out, b"{ not json").unwrap();
    assert!(!work.can_skip(&run, 0, Stage::Transcribe, "h1", &[&out]));

    // Output good, hash matches: skippable.
    std::fs::write(&out, b"[]").unwrap();
    assert!(work.can_skip(&run, 0, Stage::Transcribe, "h1", &[&out]));

    // Setting changed, so the hash differs: rerun.
    assert!(!work.can_skip(&run, 0, Stage::Transcribe, "h2", &[&out]));
    // A stage that was never done is not skippable.
    assert!(!work.can_skip(&run, 0, Stage::Translate, "h1", &[&out]));
}

#[test]
fn input_hash_changes_with_any_part() {
    let a = input_hash(&["fr", "en", "small"]);
    assert_eq!(a, input_hash(&["fr", "en", "small"]));
    assert_ne!(a, input_hash(&["fr", "en", "base"]));
    // Parts are separated, so shifting text between parts changes the hash.
    assert_ne!(input_hash(&["ab", "c"]), input_hash(&["a", "bc"]));
}

#[test]
fn work_folder_name_follows_the_project_file() {
    let dir = tempfile::tempdir().unwrap();
    let work = WorkFolder::create(&dir.path().join("My film.flick"), "p-9").unwrap();
    assert_eq!(work.path().file_name().unwrap(), "My film.flick-work");
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(work.path().join("work-manifest.json")).unwrap())
            .unwrap();
    assert_eq!(manifest["projectId"], "p-9");
}
