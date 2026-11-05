use std::collections::BTreeSet;

use serde_json::{json, Value};

use crate::cloud::adapter::{Auth, CloudAdapter, CloudResult, Languages, Voice};
use crate::cloud::http::{client, send, unexpected};
use crate::cloud::services::{Gender, Service};

/// Azure AI services: Translator v3 and Speech text to speech. One multi-service key plus a
/// region. The key goes in `Ocp-Apim-Subscription-Key`, the region in its own header.
pub struct AzureAdapter {
    service: Service,
    http: reqwest::Client,
}

impl AzureAdapter {
    pub fn new(service: Service) -> Self {
        Self::with_client(service, client())
    }

    /// With another HTTP client (tests shorten the timeout).
    pub fn with_client(service: Service, http: reqwest::Client) -> Self {
        Self { service, http }
    }

    fn url(&self, base: &str, auth: &Auth) -> String {
        self.service.url(base, auth.region)
    }

    fn with_auth(&self, req: reqwest::RequestBuilder, auth: &Auth) -> reqwest::RequestBuilder {
        let req = req.header("Ocp-Apim-Subscription-Key", auth.key.expose());
        match auth.region {
            Some(r) => req.header("Ocp-Apim-Subscription-Region", r),
            None => req,
        }
    }
}

/// App code to the code Azure Translator uses.
fn to_vendor(code: &str) -> &str {
    match code {
        "zh" => "zh-Hans",
        "no" => "nb",
        other => other,
    }
}

/// Azure Translator code to the app's code (`zh-Hans` to `zh`).
fn from_vendor(code: &str) -> String {
    match code {
        "zh-Hans" => "zh".into(),
        "nb" => "no".into(),
        other => other
            .split('-')
            .next()
            .unwrap_or(other)
            .to_ascii_lowercase(),
    }
}

fn locale_of_voice(name: &str) -> String {
    let mut parts = name.split('-');
    match (parts.next(), parts.next()) {
        (Some(l), Some(r)) => format!("{l}-{r}"),
        _ => "en-US".into(),
    }
}

fn xml_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

impl CloudAdapter for AzureAdapter {
    fn check<'a>(&'a self, auth: &'a Auth<'a>) -> CloudResult<'a, Languages> {
        Box::pin(async move {
            // A token request proves the key and the region.
            let url = self.url(&self.service.endpoints.check, auth);
            send(self.with_auth(self.http.post(url), auth).body(""), auth).await?;

            let mut translate = self.service.languages.translate.clone();
            if let Some(base) = &self.service.endpoints.languages {
                let list: Value = send(self.http.get(self.url(base, auth)), auth)
                    .await?
                    .json()
                    .await
                    .map_err(|_| unexpected("the language list was not JSON", auth))?;
                if let Some(map) = list["translation"].as_object() {
                    translate = map
                        .keys()
                        .map(|k| from_vendor(k))
                        .collect::<BTreeSet<_>>()
                        .into_iter()
                        .collect();
                }
            }
            let mut speak = self.service.languages.speak.clone();
            if let Some(base) = &self.service.endpoints.voices {
                let voices: Value = send(
                    self.with_auth(self.http.get(self.url(base, auth)), auth),
                    auth,
                )
                .await?
                .json()
                .await
                .map_err(|_| unexpected("the voice list was not JSON", auth))?;
                if let Some(all) = voices.as_array() {
                    speak = all
                        .iter()
                        .filter_map(|v| v["Locale"].as_str())
                        .map(from_vendor)
                        .collect::<BTreeSet<_>>()
                        .into_iter()
                        .collect();
                }
            }
            Ok(Languages { translate, speak })
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
            let base =
                self.service.endpoints.translate.as_deref().ok_or_else(|| {
                    unexpected("translation is not set up for this service", auth)
                })?;
            let url = format!(
                "{}/translate?api-version=3.0&from={}&to={}",
                self.url(base, auth).trim_end_matches('/'),
                to_vendor(source),
                to_vendor(target)
            );
            let body: Vec<Value> = texts.iter().map(|t| json!({ "Text": t })).collect();
            let req = self.with_auth(self.http.post(url), auth).json(&body);
            let reply: Value = send(req, auth)
                .await?
                .json()
                .await
                .map_err(|_| unexpected("the reply was not JSON", auth))?;
            let items = reply
                .as_array()
                .filter(|a| a.len() == texts.len())
                .ok_or_else(|| {
                    unexpected(
                        &format!("asked for {} lines, got another count", texts.len()),
                        auth,
                    )
                })?;
            items
                .iter()
                .map(|i| {
                    i["translations"][0]["text"]
                        .as_str()
                        .map(str::to_string)
                        .ok_or_else(|| unexpected("a line had no translation", auth))
                })
                .collect()
        })
    }

    fn voices<'a>(&'a self, auth: &'a Auth<'a>, language: &'a str) -> CloudResult<'a, Vec<Voice>> {
        Box::pin(async move {
            let base =
                self.service.endpoints.voices.as_deref().ok_or_else(|| {
                    unexpected("the voice list is not set up for this service", auth)
                })?;
            let list: Value = send(
                self.with_auth(self.http.get(self.url(base, auth)), auth),
                auth,
            )
            .await?
            .json()
            .await
            .map_err(|_| unexpected("the voice list was not JSON", auth))?;
            let all = list
                .as_array()
                .ok_or_else(|| unexpected("the voice list was not a list", auth))?;
            Ok(all
                .iter()
                .filter(|v| v["Locale"].as_str().map(from_vendor).as_deref() == Some(language))
                .filter_map(|v| {
                    let gender = match v["Gender"].as_str()? {
                        "Female" => Gender::Female,
                        "Male" => Gender::Male,
                        _ => return None,
                    };
                    Some(Voice {
                        id: v["ShortName"].as_str()?.to_string(),
                        gender,
                        language: language.to_string(),
                    })
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
            let base = self
                .service
                .endpoints
                .speak
                .as_deref()
                .ok_or_else(|| unexpected("speech is not set up for this service", auth))?;
            let ssml = format!(
                "<speak version='1.0' xml:lang='{}'><voice name='{}'>{}</voice></speak>",
                locale_of_voice(voice),
                xml_escape(voice),
                xml_escape(text)
            );
            let req = self
                .with_auth(self.http.post(self.url(base, auth)), auth)
                .header("Content-Type", "application/ssml+xml")
                .header("X-Microsoft-OutputFormat", "riff-24khz-16bit-mono-pcm")
                .header("User-Agent", "FlickFluent")
                .body(ssml);
            let bytes = send(req, auth)
                .await?
                .bytes()
                .await
                .map_err(|_| unexpected("the audio could not be read", auth))?;
            if bytes.starts_with(b"RIFF") {
                Ok(bytes.to_vec())
            } else {
                Err(unexpected("the audio was not a WAV file", auth))
            }
        })
    }
}
