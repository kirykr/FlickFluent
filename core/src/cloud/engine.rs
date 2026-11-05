//! Runs one heavy step on a cloud service: translation in batches, speech line by line. Every
//! finished batch and clip is kept on disk, so a stop (a limit, a lost connection, the user)
//! costs at most the piece in flight. Only the text of the step is ever sent.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};

use super::adapter::{CloudAdapter, Voice};
use super::clock::{Clock, Sleeper};
use super::keys::{KeyBook, KeyState};
use super::rotation::{KeyPool, PoolEvent, PoolFailure, PoolKey};
use super::services::{Gender, Service, Step};
use super::{read_json, voices_cache_dir, write_json};
use crate::error::*;
use crate::pipeline::ffmpeg::Ffmpeg;
use crate::project::dub::CloudUsage;

/// A clip is sped up at most this much to fit its slot, like the local voices.
pub const MAX_SPEEDUP: f64 = 1.3;

pub type Events<'a> = &'a mut (dyn FnMut(PoolEvent) + Send);

pub struct SpeakJob {
    pub id: String,
    pub text: String,
    /// App language code of the text.
    pub language: String,
    pub gender: Gender,
    /// The slot the clip should fit.
    pub target_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Clip {
    pub id: String,
    pub audio_ms: u64,
}

#[derive(Serialize, Deserialize, Default)]
struct Partial {
    input_hash: String,
    translated: Vec<String>,
}

pub struct CloudEngine {
    pub step: Step,
    service: Service,
    adapter: Arc<dyn CloudAdapter>,
    pool: KeyPool,
    cache_dir: Option<PathBuf>,
    voices: HashMap<String, Vec<Voice>>,
    usage: CloudUsage,
}

impl CloudEngine {
    pub fn new(
        step: Step,
        service: Service,
        adapter: Arc<dyn CloudAdapter>,
        pool: KeyPool,
        cache_dir: Option<PathBuf>,
    ) -> Self {
        let usage = CloudUsage {
            step: step.as_str().to_string(),
            service_id: service.id.clone(),
            service_name: service.name.clone(),
            ..CloudUsage::default()
        };
        Self {
            step,
            service,
            adapter,
            pool,
            cache_dir,
            voices: HashMap::new(),
            usage,
        }
    }

    /// Build the engine from the user's verified keys for a service, in their order. Fails with
    /// `CLOUD_NO_KEY` when there is none, and never reaches for another service's key.
    #[allow(clippy::too_many_arguments)]
    pub fn from_book(
        book: &KeyBook,
        app_data: &Path,
        service_id: &str,
        step: Step,
        adapter: Arc<dyn CloudAdapter>,
        clock: Arc<dyn Clock>,
        sleeper: Arc<dyn Sleeper>,
    ) -> Result<Self, AppError> {
        let service = super::services::find(book.services(), service_id)
            .filter(|s| s.offers(step))
            .ok_or_else(|| AppError::new(INVALID_SETTING, "settings.error.invalid"))?
            .clone();
        let mut keys = Vec::new();
        for k in book
            .for_service(service_id)?
            .into_iter()
            .filter(|k| k.state == KeyState::Verified)
        {
            if let Some(secret) = book.secret(&k.id)? {
                keys.push(PoolKey {
                    label: k.label,
                    region: k.region,
                    secret,
                });
            }
        }
        if keys.is_empty() {
            return Err(AppError::new(CLOUD_NO_KEY, "dub.error.cloudNoKey")
                .with_details(json!({ "service": service.name })));
        }
        let pool = KeyPool::new(keys, clock, sleeper);
        Ok(Self::new(
            step,
            service,
            adapter,
            pool,
            Some(voices_cache_dir(app_data)),
        ))
    }

    pub fn service_id(&self) -> &str {
        &self.service.id
    }

    pub fn service_name(&self) -> &str {
        &self.service.name
    }

    /// What this engine has used so far in this run.
    pub fn usage(&self) -> CloudUsage {
        CloudUsage {
            keys_used: self.pool.keys_used.clone(),
            switches: self.pool.switches,
            waited_seconds: self.pool.waited_seconds,
            ..self.usage.clone()
        }
    }

    fn fail(&self, f: PoolFailure) -> AppError {
        let service = self.service.name.clone();
        match f {
            PoolFailure::Exhausted {
                retry_after_seconds,
            } => AppError::new(CLOUD_KEYS_EXHAUSTED, "dub.error.cloudKeysExhausted").with_details(
                json!({ "service": service, "retryAfterSeconds": retry_after_seconds }),
            ),
            PoolFailure::AuthFailed { keys } => {
                let labels = keys
                    .iter()
                    .map(|(l, _)| l.clone())
                    .collect::<Vec<_>>()
                    .join(", ");
                AppError::new(CLOUD_AUTH_FAILED, "dub.error.cloudAuthFailed")
                    .with_details(json!({ "service": service, "labels": labels, "keys": keys }))
            }
            PoolFailure::Offline { message } => {
                AppError::new(CLOUD_OFFLINE, "dub.error.cloudOffline")
                    .with_details(json!({ "service": service, "detail": message }))
            }
            PoolFailure::BadReply { message } => {
                AppError::new(CLOUD_BAD_REPLY, "dub.error.cloudBadReply")
                    .with_details(json!({ "service": service, "detail": message }))
            }
        }
    }

    /// Batches of at most `batchLines` lines or `batchChars` characters.
    fn batches(&self, texts: &[String]) -> Vec<std::ops::Range<usize>> {
        let (max_lines, max_chars) = (
            self.service.limits.batch_lines,
            self.service.limits.batch_chars,
        );
        let mut out = Vec::new();
        let (mut start, mut chars) = (0, 0);
        for (i, t) in texts.iter().enumerate() {
            let n = t.chars().count();
            if i > start && (i - start >= max_lines || chars + n > max_chars) {
                out.push(start..i);
                start = i;
                chars = 0;
            }
            chars += n;
        }
        if start < texts.len() {
            out.push(start..texts.len());
        }
        out
    }

    /// Translate every text, one request per batch, writing the lines finished so far to
    /// `partial` after each batch. A restart skips what the file already holds.
    pub async fn translate_batches(
        &mut self,
        source: &str,
        target: &str,
        texts: &[String],
        partial: &Path,
        on_event: Events<'_>,
        should_stop: &(dyn Fn() -> bool + Sync),
    ) -> Result<Vec<String>, AppError> {
        let input_hash: String = {
            let mut h = Sha256::new();
            h.update(self.service.id.as_bytes());
            h.update(source.as_bytes());
            h.update(target.as_bytes());
            for t in texts {
                h.update((t.len() as u64).to_le_bytes());
                h.update(t.as_bytes());
            }
            h.finalize().iter().map(|b| format!("{b:02x}")).collect()
        };
        let mut done: Vec<String> = match read_json::<Partial>(partial) {
            Ok(p) if p.input_hash == input_hash && p.translated.len() <= texts.len() => {
                p.translated
            }
            _ => Vec::new(),
        };
        let remaining = &texts[done.len()..];
        for range in self.batches(remaining) {
            if should_stop() {
                return Err(AppError::new(CANCELLED, "dub.error.cancelled"));
            }
            let batch = &remaining[range];
            let chars: u64 = batch.iter().map(|t| t.chars().count() as u64).sum();
            let adapter = self.adapter.clone();
            let out = self
                .pool
                .call(
                    |key, region| {
                        let adapter = adapter.clone();
                        async move {
                            let auth = super::adapter::Auth {
                                key: &key,
                                region: region.as_deref(),
                            };
                            adapter.translate(&auth, source, target, batch).await
                        }
                    },
                    on_event,
                )
                .await
                .map_err(|f| self.fail(f))?;
            self.usage.chars += chars;
            self.usage.requests += 1;
            done.extend(out);
            write_json(
                partial,
                &Partial {
                    input_hash: input_hash.clone(),
                    translated: done.clone(),
                },
            )?;
        }
        Ok(done)
    }

    async fn voices_for(
        &mut self,
        language: &str,
        on_event: Events<'_>,
    ) -> Result<Vec<Voice>, AppError> {
        if let Some(v) = self.voices.get(language) {
            return Ok(v.clone());
        }
        let cache = self
            .cache_dir
            .as_ref()
            .map(|d| d.join(format!("{}-voices-{language}.json", self.service.id)));
        if let Some(path) = &cache {
            if let Ok(list) = read_json::<Vec<Voice>>(path) {
                if !list.is_empty() {
                    self.voices.insert(language.to_string(), list.clone());
                    return Ok(list);
                }
            }
        }
        let adapter = self.adapter.clone();
        let list = self
            .pool
            .call(
                |key, region| {
                    let adapter = adapter.clone();
                    async move {
                        let auth = super::adapter::Auth {
                            key: &key,
                            region: region.as_deref(),
                        };
                        adapter.voices(&auth, language).await
                    }
                },
                on_event,
            )
            .await
            .map_err(|f| self.fail(f))?;
        if let Some(path) = &cache {
            let _ = write_json(path, &list);
        }
        self.voices.insert(language.to_string(), list.clone());
        Ok(list)
    }

    /// Speak every line into `clips_dir/<id>.wav`, fitted to its slot. A clip that already
    /// exists is kept and not requested again.
    pub async fn speak_lines(
        &mut self,
        jobs: &[SpeakJob],
        clips_dir: &Path,
        ffmpeg: &Ffmpeg,
        on_event: Events<'_>,
        should_stop: &(dyn Fn() -> bool + Sync),
    ) -> Result<Vec<Clip>, AppError> {
        std::fs::create_dir_all(clips_dir).map_err(AppError::io)?;
        let mut clips = Vec::new();
        for job in jobs {
            if should_stop() {
                return Err(AppError::new(CANCELLED, "dub.error.cancelled"));
            }
            let out = clips_dir.join(format!("{}.wav", job.id));
            if let Some(ms) = super::audio::wav_duration_ms(&out) {
                clips.push(Clip {
                    id: job.id.clone(),
                    audio_ms: ms,
                });
                continue;
            }
            let voices = self.voices_for(&job.language, on_event).await?;
            let voice = voices
                .iter()
                .find(|v| v.gender == job.gender)
                .or_else(|| voices.first())
                .ok_or_else(|| {
                    AppError::new(CLOUD_BAD_REPLY, "dub.error.cloudBadReply")
                        .with_details(json!({ "service": self.service.name, "detail": "no voice for this language" }))
                })?
                .clone();
            let adapter = self.adapter.clone();
            let wav = self
                .pool
                .call(
                    |key, region| {
                        let (adapter, voice) = (adapter.clone(), voice.id.clone());
                        async move {
                            let auth = super::adapter::Auth {
                                key: &key,
                                region: region.as_deref(),
                            };
                            adapter.speak(&auth, &voice, &job.language, &job.text).await
                        }
                    },
                    on_event,
                )
                .await
                .map_err(|f| self.fail(f))?;
            self.usage.chars += job.text.chars().count() as u64;
            self.usage.requests += 1;

            let raw = clips_dir.join(format!("{}.raw.wav", job.id));
            crate::fsutil::write_atomic(&raw, &wav)?;
            let spoken = super::audio::wav_duration_ms_of(&wav).unwrap_or(0);
            let speed = if job.target_ms > 0 && spoken > job.target_ms {
                (spoken as f64 / job.target_ms as f64).min(MAX_SPEEDUP)
            } else {
                1.0
            };
            if speed > 1.001 {
                let part = clips_dir.join(format!("{}.part.wav", job.id));
                let filter = format!("atempo={speed:.4}");
                ffmpeg
                    .run(
                        &[
                            "-i".as_ref(),
                            raw.as_os_str(),
                            "-filter:a".as_ref(),
                            filter.as_ref(),
                            "-ar".as_ref(),
                            "24000".as_ref(),
                            "-ac".as_ref(),
                            "1".as_ref(),
                            part.as_os_str(),
                        ],
                        spoken,
                        &mut |_| {},
                    )
                    .await?;
                std::fs::rename(&part, &out).map_err(AppError::io)?;
                let _ = std::fs::remove_file(&raw);
            } else {
                std::fs::rename(&raw, &out).map_err(AppError::io)?;
            }
            let audio_ms = super::audio::wav_duration_ms(&out).unwrap_or(spoken);
            clips.push(Clip {
                id: job.id.clone(),
                audio_ms,
            });
        }
        Ok(clips)
    }
}
