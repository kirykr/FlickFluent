//! What the editor needs from the machine to re-make speech: the voice for a speaker, the engine
//! for a step, the models, the work folder and the cloud runtime. Built from the same plan a run
//! uses (`pipeline::run::plan_for_dub`), so the editor and a run never disagree (research R17).

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;

use serde_json::json;

use super::hash::speech_identity;
use crate::cloud::runtime::CloudRuntime;
use crate::cloud::services::Step as CloudStep;
use crate::error::{AppError, MODELS_MISSING};
use crate::pipeline::plan::{ModelPlan, VoicePick};
use crate::pipeline::run::{plan_for_dub, voice_pick, RunConfig};
use crate::pipeline::state::WorkFolder;
use crate::project::dub::{DubData, Speaker};

pub struct EditorRuntime {
    cfg: RunConfig,
    plan: ModelPlan,
    speakers: Vec<Speaker>,
    translate: Option<String>,
    speak: Option<String>,
    subtitles_only: bool,
    target_language: String,
}

impl EditorRuntime {
    /// `dub.options.fallbacks.engines` must already hold the engines to use. The shell merges the
    /// settings into it first, as it does for a run.
    pub fn for_project(cfg: RunConfig, dub: &DubData) -> Self {
        let (plan, _) = plan_for_dub(
            &cfg.catalogue,
            cfg.class,
            &cfg.os,
            &cfg.arch,
            &cfg.models_dir,
            dub,
        );
        let engines = &dub.options.fallbacks.engines;
        Self {
            plan,
            speakers: dub.speakers.clone(),
            translate: engines.cloud(CloudStep::Translate).map(String::from),
            speak: engines.cloud(CloudStep::Speak).map(String::from),
            subtitles_only: dub.options.fallbacks.subtitles_only,
            target_language: dub.target_language.clone(),
            cfg,
        }
    }

    /// A subtitles-only project makes no speech, so no line needs a clip.
    pub fn speaks(&self) -> bool {
        !self.subtitles_only
    }

    pub fn target_language(&self) -> &str {
        &self.target_language
    }

    pub fn config(&self) -> &RunConfig {
        &self.cfg
    }

    pub fn plan(&self) -> &ModelPlan {
        &self.plan
    }

    /// The speakers as the editor was opened with. A gender change goes through `set_speakers`.
    pub fn set_speakers(&mut self, speakers: &[Speaker]) {
        self.speakers = speakers.to_vec();
    }

    pub fn voice_for(&self, speaker_id: &str) -> Option<VoicePick> {
        self.voice_in(&self.speakers, speaker_id)
    }

    /// Like `voice_for`, with the speakers as the session holds them now (a gender may have changed).
    pub fn voice_in(&self, speakers: &[Speaker], speaker_id: &str) -> Option<VoicePick> {
        voice_pick(&self.plan, speakers, speaker_id)
    }

    /// The service id when speech goes to a cloud service.
    pub fn cloud_speak(&self) -> Option<&str> {
        self.speak.as_deref().filter(|_| !self.subtitles_only)
    }

    /// `local:<model>` or `cloud:<service>` for the translate step or, for speak, the first voice.
    pub fn engine_id(&self, step: CloudStep) -> String {
        match step {
            CloudStep::Translate => match &self.translate {
                Some(id) => format!("cloud:{id}"),
                None => format!("local:{}", self.plan.translate.clone().unwrap_or_default()),
            },
            CloudStep::Speak => match self.cloud_speak() {
                Some(id) => format!("cloud:{id}"),
                None => format!(
                    "local:{}",
                    self.plan
                        .voices
                        .values()
                        .next()
                        .map(|v| v.model.clone())
                        .unwrap_or_default()
                ),
            },
        }
    }

    /// The engine that would speak this speaker's lines.
    pub fn speech_engine_id(&self, speaker_id: &str) -> String {
        self.speech_engine_in(&self.speakers, speaker_id)
    }

    pub fn speech_engine_in(&self, speakers: &[Speaker], speaker_id: &str) -> String {
        self.identity(speakers, speaker_id).1
    }

    /// A string naming the voice for a speaker, for the speech hash. A cloud step has no voice
    /// of ours, so the speaker's gender decides.
    pub fn voice_id(&self, speakers: &[Speaker], speaker_id: &str) -> String {
        self.identity(speakers, speaker_id).0
    }

    fn identity(&self, speakers: &[Speaker], speaker_id: &str) -> (String, String) {
        speech_identity(&self.plan, speakers, self.cloud_speak(), speaker_id)
    }

    /// The work folder of this project, or `None` when it is gone.
    pub fn work_folder(&self) -> Option<PathBuf> {
        WorkFolder::open_for(&self.cfg.project_path, &self.cfg.project_id)
            .map(|w| w.path().to_path_buf())
    }

    pub fn cloud(&self) -> Option<&Arc<CloudRuntime>> {
        self.cfg.cloud.as_ref()
    }

    /// Every local voice model these speakers need must be installed. Names the missing ones.
    pub fn check_models(&self, speakers: &[&str]) -> Result<(), AppError> {
        if self.cloud_speak().is_some() {
            return Ok(());
        }
        let mut wanted = BTreeSet::new();
        for speaker in speakers {
            match self.voice_for(speaker) {
                Some(v) => {
                    wanted.insert(v.model);
                }
                None => {
                    return Err(AppError::new(MODELS_MISSING, "dub.error.modelsMissing")
                        .with_details(json!({ "missing": [] })))
                }
            }
        }
        let missing: Vec<String> = wanted
            .into_iter()
            .filter(|id| !self.cfg.models_dir.join(id).exists())
            .collect();
        if missing.is_empty() {
            Ok(())
        } else {
            Err(AppError::new(MODELS_MISSING, "dub.error.modelsMissing")
                .with_details(json!({ "missing": missing })))
        }
    }
}
