#![allow(dead_code)] // each test file uses a different part of this module
//! An editing session over a hand-built project saved to a real project file, so a test can
//! compare the file on disk with the session after every edit.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use flickfluent_core::cloud::runtime::CloudRuntime;
use flickfluent_core::editor::runtime::EditorRuntime;
use flickfluent_core::editor::session::{EditSession, Saver};
use flickfluent_core::pipeline::run::{run_dub, CancelToken, Mode};
use flickfluent_core::project::dub::{
    read_from, write_into, DubData, PipelineRun, Section, Speaker, SpeakerGender, TranscriptLine,
};
use flickfluent_core::project::{Project, ProjectSession};
use serde_json::{json, Value};

use super::run_fixture::{config, dub_for};

pub struct Editing {
    pub root: tempfile::TempDir,
    pub path: PathBuf,
    pub session: EditSession,
    shared: Arc<Mutex<ProjectSession>>,
}

pub fn speaker(id: &str, gender: SpeakerGender) -> Speaker {
    Speaker {
        id: id.into(),
        gender,
        ..Default::default()
    }
}

/// A line with a clip. `id` doubles as the clip name.
pub fn line(id: &str, speaker: &str, start_ms: u64, end_ms: u64, text: &str) -> TranscriptLine {
    TranscriptLine {
        id: id.into(),
        section_index: 0,
        start_ms,
        end_ms,
        speaker_id: speaker.into(),
        language: "fr".into(),
        text: format!("orig {id}"),
        translated: Some(text.into()),
        audio: Some(format!("sections/0/clips/{id}.wav")),
        audio_ms: Some(end_ms - start_ms),
        ..Default::default()
    }
}

/// Two speakers, one section of 60 s, four lines of 2 s each with a second between them.
pub fn dub() -> DubData {
    DubData {
        source_language: "fr".into(),
        target_language: "en".into(),
        speakers: vec![
            speaker("spk1", SpeakerGender::Male),
            speaker("spk2", SpeakerGender::Female),
        ],
        lines: vec![
            line("l1", "spk1", 1_000, 3_000, "Hello"),
            line("l2", "spk2", 4_000, 6_000, "Good morning"),
            line("l3", "spk1", 7_000, 9_000, "How are you"),
            line("l4", "spk2", 10_000, 12_000, "Goodbye"),
        ],
        run: Some(PipelineRun {
            id: "r1".into(),
            sections: vec![Section {
                index: 0,
                start_ms: 0,
                end_ms: 60_000,
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    }
}

pub fn saver(shared: Arc<Mutex<ProjectSession>>) -> Saver {
    Box::new(move |dub: &DubData| {
        let mut s = shared.lock().unwrap();
        let mut data = s.project.data.clone();
        write_into(&mut data, dub);
        s.set_data(data);
        s.save()
    })
}

impl Editing {
    pub fn new(dub: DubData) -> Self {
        Self::with(dub, |_| {})
    }

    /// `tweak` may change the project data before the session opens (an engine, a flag).
    pub fn with(dub: DubData, tweak: impl FnOnce(&mut DubData)) -> Self {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("a.flick");
        let mut dub = dub;
        tweak(&mut dub);
        let mut project = ProjectSession::create("A", &path).unwrap();
        let mut data = json!({});
        write_into(&mut data, &dub);
        project.set_data(data);
        project.save().unwrap();
        let shared = Arc::new(Mutex::new(project));
        let rt = EditorRuntime::for_project(config(root.path(), "a.flick", json!({})), &dub);
        let session = EditSession::open(rt, dub, saver(shared.clone())).unwrap();
        Self {
            root,
            path,
            session,
            shared,
        }
    }

    /// The project as a fresh reader sees it on disk.
    pub fn on_disk(&self) -> DubData {
        read_from(&Project::read(&self.path).unwrap().data)
    }

    /// Disk equals memory (SC-007).
    pub fn assert_saved(&self) {
        let disk = self.on_disk();
        let mem = self.session.dub();
        assert_eq!(disk.lines, mem.lines, "lines on disk differ from memory");
        assert_eq!(
            disk.speakers, mem.speakers,
            "speakers on disk differ from memory"
        );
        assert_eq!(
            disk.run.as_ref().map(|r| &r.sections),
            mem.run.as_ref().map(|r| &r.sections)
        );
    }

    pub fn work(&self) -> &Path {
        self.root.path()
    }

    pub fn close(self) {
        drop(self.shared);
    }
}

/// A finished fake dub of a 12-minute video (several sections) with an editing session over it, shared the way
/// the shell shares it, so a re-synthesis can run while edits arrive.
pub struct Live {
    pub root: tempfile::TempDir,
    pub path: PathBuf,
    pub work: PathBuf,
    pub shared: Arc<Mutex<EditSession>>,
    pub project: Arc<Mutex<ProjectSession>>,
    pub log: PathBuf,
}

impl Live {
    /// Run the dub fast, then open a session whose sidecar uses `session_params` (a delay, say).
    pub async fn finished(session_params: Value) -> Self {
        Self::finished_with(session_params, None, None).await
    }

    /// `speak_cloud` sends speech to that service for the session (not for the finished run, which
    /// was local), the way a changed setting would.
    pub async fn finished_with(
        session_params: Value,
        cloud: Option<Arc<CloudRuntime>>,
        speak_cloud: Option<&str>,
    ) -> Self {
        let root = tempfile::tempdir().unwrap();
        let log = root.path().join("synth.log");
        let fast = json!({ "synthLog": log });
        let cfg = config(root.path(), "e.flick", fast);
        let source = super::run_fixture::long_clip(root.path(), 12);
        let dub = dub_for(&source, "en").await;
        let dub = run_dub(&cfg, dub, Mode::Fresh, CancelToken::default(), &mut |_| {})
            .await
            .unwrap();
        let _ = std::fs::remove_file(&log);

        let path = root.path().join("e.flick");
        let mut project = ProjectSession::create("E", &path).unwrap();
        let mut data = json!({});
        write_into(&mut data, &dub);
        project.set_data(data);
        project.save().unwrap();
        let project = Arc::new(Mutex::new(project));

        let mut params = session_params;
        params["synthLog"] = json!(log);
        let mut cfg = config(root.path(), "e.flick", params);
        cfg.cloud = cloud;
        let mut runtime_dub = dub.clone();
        if let Some(id) = speak_cloud {
            super::cloud_run::use_cloud(&mut runtime_dub, None, Some(id));
        }
        let rt = EditorRuntime::for_project(cfg, &runtime_dub);
        let session = EditSession::open(rt, dub, saver(project.clone())).unwrap();
        let work = flickfluent_core::pipeline::state::WorkFolder::folder_for(&path);
        Self {
            root,
            path,
            work,
            shared: Arc::new(Mutex::new(session)),
            project,
            log,
        }
    }

    pub fn with<R>(&self, f: impl FnOnce(&mut EditSession) -> R) -> R {
        f(&mut self.shared.lock().unwrap())
    }

    pub fn lines(&self) -> Vec<TranscriptLine> {
        self.with(|s| s.dub().lines.clone())
    }

    pub fn edit_text(&self, id: &str, text: &str) {
        use flickfluent_core::editor::ops::{EditOp, LinePatch};
        self.with(|s| {
            s.apply(EditOp::UpdateLine {
                id: id.into(),
                patch: LinePatch {
                    translated: Some(text.into()),
                    ..Default::default()
                },
            })
            .unwrap()
        });
    }

    /// The ids of each synthesize request the fake sidecar got since the dub finished.
    pub fn requests(&self) -> Vec<Vec<String>> {
        std::fs::read_to_string(&self.log)
            .unwrap_or_default()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    }

    /// Every file in the clip folders, by path under the work folder, with its bytes.
    pub fn clip_files(&self) -> BTreeMap<String, Vec<u8>> {
        fn walk(dir: &Path, base: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
            for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
                let p = e.path();
                if p.is_dir() {
                    walk(&p, base, out);
                } else if p.to_string_lossy().contains("clips") {
                    out.insert(
                        p.strip_prefix(base)
                            .unwrap()
                            .to_string_lossy()
                            .replace('\\', "/"),
                        std::fs::read(&p).unwrap(),
                    );
                }
            }
        }
        let mut out = BTreeMap::new();
        walk(&self.work.join("sections"), &self.work, &mut out);
        out
    }
}
