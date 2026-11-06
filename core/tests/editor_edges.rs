//! Edge cases from the spec: a missing source, a removed model, no work folder, a cloud line
//! without agreement. Text editing and subtitles never need the heavy parts.

mod support;

use flickfluent_core::cloud::services::Step;
use flickfluent_core::editor::resynth::resynthesize;
use flickfluent_core::error::{CLOUD_CONSENT_REQUIRED, EDIT_WORK_FOLDER_MISSING, MODELS_MISSING};
use flickfluent_core::pipeline::run::CancelToken;
use flickfluent_core::subtitles::write::{export_file, Format, Source};
use serde_json::json;
use support::cloud_run::cloud;
use support::editor_fixture::Live;
use support::fake_cloud::FakeCloud;

#[tokio::test]
async fn with_the_source_video_gone_text_edits_and_subtitles_still_work() {
    let live = Live::finished(json!({})).await;
    let source = live.with(|s| s.dub().source.clone().unwrap().path);
    std::fs::remove_file(&source).unwrap();
    let id = live.lines()[0].id.clone();
    live.edit_text(&id, "Still editable");
    let out = live.root.path().join("out.vtt");
    let n = export_file(&live.lines(), Source::Translated, Format::Vtt, &out, None).unwrap();
    assert_eq!(n, live.lines().len());
    assert!(std::fs::read_to_string(out)
        .unwrap()
        .contains("Still editable"));
}

#[tokio::test]
async fn a_removed_voice_model_is_named_before_any_work_starts() {
    let live = Live::finished(json!({})).await;
    let id = live.lines()[0].id.clone();
    live.edit_text(&id, "Needs a voice");
    let speaker = live.lines()[0].speaker_id.clone();
    let voice = live.with(|s| s.runtime().voice_for(&speaker).unwrap().model);
    std::fs::remove_dir_all(live.root.path().join("models").join(&voice)).unwrap();
    let err = resynthesize(&live.shared, None, CancelToken::default(), &mut |_| {})
        .await
        .unwrap_err();
    assert_eq!(err.code, MODELS_MISSING);
    assert_eq!(err.details.unwrap()["missing"], json!([voice]));
    assert!(live.requests().is_empty());
}

#[tokio::test]
async fn without_the_work_folder_everything_but_speech_works() {
    let live = Live::finished(json!({})).await;
    std::fs::remove_dir_all(&live.work).unwrap();
    let lines = live.lines();
    live.edit_text(&lines[0].id, "Text edit");
    let found = live.with(|s| {
        flickfluent_core::editor::search::find(
            &s.dub().lines,
            &flickfluent_core::editor::search::SearchOptions {
                query: "text edit".into(),
                field: flickfluent_core::editor::search::Field::Translated,
                match_case: false,
                whole_word: false,
            },
        )
    });
    assert_eq!(found.len(), 1);
    // Fitting arithmetic needs no files; only the post chain does.
    let plan = flickfluent_core::editor::fit::plan(
        &live.lines(),
        flickfluent_core::editor::fit::FitMode::Smart,
        None,
    );
    let _ = plan.report.lines.len();
    let out = live.root.path().join("o.srt");
    export_file(&live.lines(), Source::Both, Format::Srt, &out, None).unwrap();
    let err = resynthesize(&live.shared, None, CancelToken::default(), &mut |_| {})
        .await
        .unwrap_err();
    assert_eq!(err.code, EDIT_WORK_FOLDER_MISSING);
}

#[tokio::test]
async fn a_cloud_line_without_an_agreement_asks_first_and_sends_nothing() {
    let appdata = tempfile::tempdir().unwrap();
    let fake = FakeCloud::start();
    let c = cloud(
        appdata.path(),
        &fake,
        &[("google-gemini", "mine", "edge-key-0001", None)],
    );
    // No agreement is granted.
    let live = Live::finished_with(json!({}), Some(c.runtime.clone()), Some("google-gemini")).await;
    let id = live.lines()[0].id.clone();
    live.edit_text(&id, "Hello");
    let err = resynthesize(
        &live.shared,
        Some(vec![id]),
        CancelToken::default(),
        &mut |_| {},
    )
    .await
    .unwrap_err();
    assert_eq!(err.code, CLOUD_CONSENT_REQUIRED);
    assert_eq!(
        err.details.unwrap()["steps"][0]["step"],
        Step::Speak.as_str()
    );
    assert_eq!(fake.count(), 0);
}
