//! Shared HTTP helpers for the cloud adapters: one place that turns a reply or a network error
//! into a [`CloudFailure`], and keeps keys out of every message.

use std::time::Duration;

use reqwest::{Response, StatusCode};
use time::format_description::well_known::Rfc2822;

use super::adapter::{Auth, CloudFailure, FailureKind};
use super::secret::scrub;

/// Longest a single request may take. Keeps "Verify connection" well inside 30 seconds.
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(25);

pub fn client() -> reqwest::Client {
    client_with_timeout(REQUEST_TIMEOUT)
}

pub fn client_with_timeout(timeout: Duration) -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(timeout)
        .build()
        .expect("http client")
}

/// `Retry-After` as seconds, from either the seconds form or an HTTP date.
pub fn retry_after_seconds(value: &str) -> Option<u64> {
    let value = value.trim();
    if let Ok(s) = value.parse::<u64>() {
        return Some(s);
    }
    let when = time::OffsetDateTime::parse(value, &Rfc2822).ok()?;
    let secs = (when - time::OffsetDateTime::now_utc()).whole_seconds();
    Some(secs.max(0) as u64)
}

pub fn network_failure(e: &reqwest::Error, auth: &Auth) -> CloudFailure {
    let kind = if e.is_timeout() {
        FailureKind::Unavailable
    } else if e.is_connect() {
        FailureKind::Offline
    } else {
        FailureKind::Unavailable
    };
    // The error text can include the URL, never the headers; scrub anyway.
    let mut text = e.to_string();
    if let Some(url) = e.url() {
        text = text.replace(url.as_str(), "[url]");
    }
    CloudFailure::new(kind, scrub(&text, &[auth.key]))
}

fn short(text: &str) -> String {
    let t: String = text.chars().take(240).collect();
    t.trim().to_string()
}

/// Turn a non-success reply into the failure the rotation code understands.
pub async fn failure_from(resp: Response, auth: &Auth<'_>) -> CloudFailure {
    let status = resp.status();
    let retry_after = resp
        .headers()
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|v| v.to_str().ok())
        .and_then(retry_after_seconds);
    let body = resp.text().await.unwrap_or_default();
    let lower = body.to_ascii_lowercase();
    let message = scrub(&short(&body), &[auth.key]);
    let kind = match status {
        StatusCode::TOO_MANY_REQUESTS if lower.contains("billing") || lower.contains("credit") => {
            FailureKind::NoCredit
        }
        StatusCode::TOO_MANY_REQUESTS => FailureKind::RateLimited,
        StatusCode::SERVICE_UNAVAILABLE if retry_after.is_some() => FailureKind::RateLimited,
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => FailureKind::Rejected,
        StatusCode::PAYMENT_REQUIRED => FailureKind::NoCredit,
        StatusCode::BAD_REQUEST
            if lower.contains("api key not valid") || lower.contains("api_key_invalid") =>
        {
            FailureKind::Rejected
        }
        s if s.is_server_error() => FailureKind::Unavailable,
        _ => FailureKind::Unexpected,
    };
    CloudFailure::new(kind, message).retry_after(retry_after)
}

/// Send a request and return the reply when it succeeded, or the failure otherwise.
pub async fn send(req: reqwest::RequestBuilder, auth: &Auth<'_>) -> Result<Response, CloudFailure> {
    let resp = req.send().await.map_err(|e| network_failure(&e, auth))?;
    if resp.status().is_success() {
        Ok(resp)
    } else {
        Err(failure_from(resp, auth).await)
    }
}

pub fn unexpected(what: &str, auth: &Auth) -> CloudFailure {
    CloudFailure::new(FailureKind::Unexpected, scrub(what, &[auth.key]))
}

/// A 24 kHz, 16-bit, mono WAV around raw little-endian PCM.
pub fn wav_from_pcm(pcm: &[u8], rate: u32) -> Vec<u8> {
    let mut out = Vec::with_capacity(44 + pcm.len());
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + pcm.len() as u32).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&rate.to_le_bytes());
    out.extend_from_slice(&(rate * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&(pcm.len() as u32).to_le_bytes());
    out.extend_from_slice(pcm);
    out
}
