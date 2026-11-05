use base64::Engine;
use serde_json::{json, Value};

use crate::cloud::adapter::{Auth, CloudAdapter, CloudFailure, CloudResult, Languages, Voice};
use crate::cloud::http::{client, send, unexpected, wav_from_pcm};
use crate::cloud::services::Service;

/// Google Gemini API: translation by a text model with a fixed output shape, speech by the
/// text-to-speech model. The key goes in the `x-goog-api-key` header only.
pub struct GeminiAdapter {
    service: Service,
    http: reqwest::Client,
}

impl GeminiAdapter {
    pub fn new(service: Service) -> Self {
        Self::with_client(service, client())
    }

    /// With another HTTP client (tests shorten the timeout).
    pub fn with_client(service: Service, http: reqwest::Client) -> Self {
        Self { service, http }
    }

    fn model_url(&self, base: Option<&String>, model: Option<&String>) -> Option<String> {
        Some(format!(
            "{}/{}:generateContent",
            base?.trim_end_matches('/'),
            model?
        ))
    }
}

/// Language as the model should read it. Codes the model might misread get a name.
fn language_name(code: &str) -> String {
    match code {
        "zh" => "Chinese (Simplified)".into(),
        "pt" => "Portuguese".into(),
        "km" => "Khmer".into(),
        other => other.to_string(),
    }
}

fn first_text(reply: &Value) -> Option<&str> {
    reply["candidates"][0]["content"]["parts"][0]["text"].as_str()
}

impl CloudAdapter for GeminiAdapter {
    fn check<'a>(&'a self, auth: &'a Auth<'a>) -> CloudResult<'a, Languages> {
        Box::pin(async move {
            let req = self
                .http
                .get(&self.service.endpoints.check)
                .header("x-goog-api-key", auth.key.expose());
            let reply: Value = send(req, auth)
                .await?
                .json()
                .await
                .map_err(|_| unexpected("the reply was not JSON", auth))?;
            if !reply["models"].is_array() {
                return Err(unexpected("the reply had no model list", auth));
            }
            // Gemini has no language list to ask for; the data file's list stands.
            Ok(Languages {
                translate: self.service.languages.translate.clone(),
                speak: self.service.languages.speak.clone(),
            })
        })
    }

    fn translate<'a>(
        &'a self,
        auth: &'a Auth<'a>,
        source: &'a str,
        target: &'a str,
        texts: &'a [String],
    ) -> CloudResult<'a, Vec<String>> {
        Box::pin(async move {
            let url = self
                .model_url(
                    self.service.endpoints.translate.as_ref(),
                    self.service.models.translate.as_ref(),
                )
                .ok_or_else(|| unexpected("translation is not set up for this service", auth))?;
            let prompt = format!(
                "Translate each line from {} to {}. Keep names and numbers unchanged. \
                 Return only a JSON array with exactly {} strings, one translation per line, in the \
                 same order, with no commentary.\nLINES:\n{}",
                language_name(source),
                language_name(target),
                texts.len(),
                serde_json::to_string(texts).unwrap_or_default()
            );
            let body = json!({
                "contents": [{ "parts": [{ "text": prompt }] }],
                "generationConfig": {
                    "responseMimeType": "application/json",
                    "responseSchema": { "type": "ARRAY", "items": { "type": "STRING" } },
                    "temperature": 0
                }
            });
            let req = self
                .http
                .post(url)
                .header("x-goog-api-key", auth.key.expose())
                .json(&body);
            let reply: Value = send(req, auth)
                .await?
                .json()
                .await
                .map_err(|_| unexpected("the reply was not JSON", auth))?;
            let text =
                first_text(&reply).ok_or_else(|| unexpected("the reply had no text", auth))?;
            let lines: Vec<String> = serde_json::from_str(text)
                .map_err(|_| unexpected("the reply was not a list of lines", auth))?;
            if lines.len() != texts.len() {
                return Err(unexpected(
                    &format!("asked for {} lines, got {}", texts.len(), lines.len()),
                    auth,
                ));
            }
            Ok(lines)
        })
    }

    fn voices<'a>(&'a self, _auth: &'a Auth<'a>, language: &'a str) -> CloudResult<'a, Vec<Voice>> {
        Box::pin(async move {
            // The curated list in the data file; the spoken language follows the text.
            Ok(self
                .service
                .voices
                .iter()
                .flatten()
                .map(|v| Voice {
                    id: v.id.clone(),
                    gender: v.gender,
                    language: language.to_string(),
                })
                .collect())
        })
    }

    fn speak<'a>(
        &'a self,
        auth: &'a Auth<'a>,
        voice: &'a str,
        _language: &'a str,
        text: &'a str,
    ) -> CloudResult<'a, Vec<u8>> {
        Box::pin(async move {
            let url = self
                .model_url(
                    self.service.endpoints.speak.as_ref(),
                    self.service.models.speak.as_ref(),
                )
                .ok_or_else(|| unexpected("speech is not set up for this service", auth))?;
            let body = json!({
                "contents": [{ "parts": [{ "text": text }] }],
                "generationConfig": {
                    "responseModalities": ["AUDIO"],
                    "speechConfig": { "voiceConfig": { "prebuiltVoiceConfig": { "voiceName": voice } } }
                }
            });
            let req = self
                .http
                .post(url)
                .header("x-goog-api-key", auth.key.expose())
                .json(&body);
            let reply: Value = send(req, auth)
                .await?
                .json()
                .await
                .map_err(|_| unexpected("the reply was not JSON", auth))?;
            let part = &reply["candidates"][0]["content"]["parts"][0]["inlineData"];
            let data = part["data"]
                .as_str()
                .ok_or_else(|| unexpected("the reply had no audio", auth))?;
            let mime = part["mimeType"]
                .as_str()
                .unwrap_or_default()
                .to_ascii_lowercase();
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(data)
                .map_err(|_| unexpected("the audio was not valid", auth))?;
            if bytes.starts_with(b"RIFF") {
                Ok(bytes)
            } else if mime.contains("l16") || mime.contains("pcm") {
                // Raw 16-bit little-endian PCM; the rate is in the type, 24 kHz when absent.
                let rate = mime
                    .split("rate=")
                    .nth(1)
                    .and_then(|r| r.split(';').next())
                    .and_then(|r| r.trim().parse().ok())
                    .unwrap_or(24000);
                Ok(wav_from_pcm(&bytes, rate))
            } else {
                Err::<Vec<u8>, CloudFailure>(unexpected("the audio type was not recognised", auth))
            }
        })
    }
}
