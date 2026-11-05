#![allow(dead_code)] // each test file uses a different part of this module
//! A local HTTP server that imitates the Google Gemini and Azure endpoints the cloud adapters
//! call (`specs/003-hybrid-cloud-keys/contracts/cloud-adapter.md`). It answers by key, so a test
//! can make the first key fail while the second works. It counts requests and keeps every
//! request's path, headers and body, so a test can prove where a key was and was not sent.
//!
//! Paths: `/gemini/models`, `/gemini/models/<model>:generateContent`, `/azure/sts`,
//! `/azure/translate`, `/azure/speak`, `/azure/voices`, `/azure/languages`.
//! A Gemini translation request ends its text with `LINES:` and a JSON list of the lines.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use base64::Engine;

#[derive(Clone, Debug)]
pub enum Mode {
    Ok,
    /// Answer 429 for the next `times` requests with this key, then behave as `Ok`.
    RateLimit {
        times: u32,
        retry_after: Option<u64>,
    },
    Reject,
    NoCredit,
    /// Wait this many milliseconds, then answer as `Ok`.
    Slow(u64),
    /// Answer 500.
    ServerError,
    /// Close the connection without an answer.
    Drop,
    /// Answer 200 with a body that is not JSON.
    Junk,
    /// Translate with one line fewer than asked.
    WrongCount,
}

#[derive(Clone, Debug)]
pub struct Logged {
    /// Path and query exactly as sent.
    pub path: String,
    /// Lower-case header names.
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

impl Logged {
    pub fn key(&self) -> Option<&String> {
        self.headers
            .get("x-goog-api-key")
            .or_else(|| self.headers.get("ocp-apim-subscription-key"))
    }
    pub fn body_text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }
}

#[derive(Default)]
struct State {
    default_mode: Option<Mode>,
    by_key: HashMap<String, Mode>,
    log: Vec<Logged>,
    in_flight: HashMap<String, u32>,
    max_in_flight: HashMap<String, u32>,
}

pub struct FakeCloud {
    pub base: String,
    state: Arc<Mutex<State>>,
}

impl FakeCloud {
    pub fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let state = Arc::new(Mutex::new(State::default()));
        let shared = state.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let shared = shared.clone();
                std::thread::spawn(move || handle(stream, shared));
            }
        });
        Self { base, state }
    }

    pub fn url(&self, path: &str) -> String {
        format!("{}{}", self.base, path)
    }
    pub fn gemini_models_url(&self) -> String {
        self.url("/gemini/models")
    }
    pub fn azure_url(&self, path: &str) -> String {
        self.url(&format!("/azure{path}"))
    }

    /// Behaviour for every key that has none of its own.
    pub fn set_default(&self, mode: Mode) {
        self.state.lock().unwrap().default_mode = Some(mode);
    }
    /// Back to answering `Ok` for every key.
    pub fn reset(&self) {
        let mut st = self.state.lock().unwrap();
        st.default_mode = None;
        st.by_key.clear();
    }
    pub fn set_for_key(&self, key: &str, mode: Mode) {
        self.state
            .lock()
            .unwrap()
            .by_key
            .insert(key.to_string(), mode);
    }
    pub fn requests(&self) -> Vec<Logged> {
        self.state.lock().unwrap().log.clone()
    }
    pub fn count(&self) -> usize {
        self.state.lock().unwrap().log.len()
    }
    /// The most requests that were ever being answered at once for this key.
    pub fn max_in_flight_for_key(&self, key: &str) -> u32 {
        self.state
            .lock()
            .unwrap()
            .max_in_flight
            .get(key)
            .copied()
            .unwrap_or(0)
    }
    /// Requests made with one key.
    pub fn count_for_key(&self, key: &str) -> usize {
        self.requests()
            .iter()
            .filter(|r| r.key().map(String::as_str) == Some(key))
            .count()
    }
}

/// A short 24 kHz, 16-bit, mono WAV (a quiet tone), the shape both services return.
pub fn wav_24k(ms: u32) -> Vec<u8> {
    let samples = 24 * ms as usize;
    let mut pcm = Vec::with_capacity(samples * 2);
    for i in 0..samples {
        let v = ((i as f32 * 0.05).sin() * 2000.0) as i16;
        pcm.extend_from_slice(&v.to_le_bytes());
    }
    let mut out = Vec::with_capacity(44 + pcm.len());
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + pcm.len() as u32).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&24000u32.to_le_bytes());
    out.extend_from_slice(&48000u32.to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&(pcm.len() as u32).to_le_bytes());
    out.extend_from_slice(&pcm);
    out
}

fn handle(stream: TcpStream, state: Arc<Mutex<State>>) {
    let mut key_seen = String::new();
    handle_inner(stream, state.clone(), &mut key_seen);
    if !key_seen.is_empty() {
        if let Some(n) = state.lock().unwrap().in_flight.get_mut(&key_seen) {
            *n = n.saturating_sub(1);
        }
    }
}

fn handle_inner(mut stream: TcpStream, state: Arc<Mutex<State>>, key_seen: &mut String) {
    stream.set_read_timeout(Some(Duration::from_secs(5))).ok();
    let Some((method, path, headers, body)) = read_request(&mut stream) else {
        return;
    };
    let key = headers
        .get("x-goog-api-key")
        .or_else(|| headers.get("ocp-apim-subscription-key"))
        .cloned()
        .unwrap_or_default();
    *key_seen = key.clone();
    let mode = {
        let mut st = state.lock().unwrap();
        st.log.push(Logged {
            path: path.clone(),
            headers: headers.clone(),
            body: body.clone(),
        });
        let now = {
            let n = st.in_flight.entry(key.clone()).or_insert(0);
            *n += 1;
            *n
        };
        let max = st.max_in_flight.entry(key.clone()).or_insert(0);
        *max = (*max).max(now);
        let mode = st
            .by_key
            .get(&key)
            .cloned()
            .or_else(|| st.default_mode.clone())
            .unwrap_or(Mode::Ok);
        if let Mode::RateLimit { times, retry_after } = mode {
            if times <= 1 {
                st.by_key.insert(key.clone(), Mode::Ok);
            } else {
                st.by_key.insert(
                    key.clone(),
                    Mode::RateLimit {
                        times: times - 1,
                        retry_after,
                    },
                );
            }
        }
        mode
    };

    match mode {
        Mode::Drop => {
            let _ = stream.shutdown(Shutdown::Both);
            return;
        }
        Mode::RateLimit { retry_after, .. } => {
            let extra = retry_after
                .map(|s| format!("Retry-After: {s}\r\n"))
                .unwrap_or_default();
            respond(
                &mut stream,
                429,
                &extra,
                "application/json",
                br#"{"error":{"message":"too many requests"}}"#,
            );
            return;
        }
        Mode::Reject => {
            let code = if path.starts_with("/gemini") {
                403
            } else {
                401
            };
            // Like some real services, the refusal text repeats the key it was given.
            let text = format!(r#"{{"error":{{"message":"the key {key} was refused"}}}}"#);
            respond(&mut stream, code, "", "application/json", text.as_bytes());
            return;
        }
        Mode::ServerError => {
            respond(
                &mut stream,
                500,
                "",
                "application/json",
                br#"{"error":{"message":"the service failed"}}"#,
            );
            return;
        }
        Mode::NoCredit => {
            respond(
                &mut stream,
                402,
                "",
                "application/json",
                br#"{"error":{"message":"quota or billing problem"}}"#,
            );
            return;
        }
        Mode::Slow(ms) => std::thread::sleep(Duration::from_millis(ms)),
        Mode::Junk => {
            respond(&mut stream, 200, "", "application/json", b"not json{");
            return;
        }
        Mode::Ok | Mode::WrongCount => {}
    }

    let wrong_count = matches!(mode, Mode::WrongCount);
    let (status, ctype, out) = route(&method, &path, &body, wrong_count);
    respond(&mut stream, status, "", ctype, &out);
}

fn route(method: &str, path: &str, body: &[u8], wrong_count: bool) -> (u16, &'static str, Vec<u8>) {
    let json = "application/json";
    let route_path = path.split('?').next().unwrap_or(path);
    match (method, route_path) {
        ("GET", "/gemini/models") => (
            200,
            json,
            br#"{"models":[{"name":"models/gemini-3.8-flash"},{"name":"models/gemini-3.8-flash-tts"}]}"#.to_vec(),
        ),
        ("POST", p) if p.starts_with("/gemini/models/") && p.ends_with(":generateContent") => {
            let text = String::from_utf8_lossy(body);
            let v: serde_json::Value = serde_json::from_slice(body).unwrap_or_default();
            let wants_audio = v["generationConfig"]["responseModalities"]
                .as_array()
                .is_some_and(|m| m.iter().any(|x| x == "AUDIO"));
            if wants_audio {
                let data = base64::engine::general_purpose::STANDARD.encode(wav_24k(200));
                let reply = serde_json::json!({"candidates":[{"content":{"parts":[{"inlineData":{"mimeType":"audio/wav","data":data}}]}}]});
                return (200, json, reply.to_string().into_bytes());
            }
            let prompt = v["contents"][0]["parts"][0]["text"].as_str().unwrap_or(&text);
            let lines: Vec<String> = prompt
                .split("LINES:")
                .nth(1)
                .and_then(|l| serde_json::from_str(l.trim()).ok())
                .unwrap_or_default();
            let mut out: Vec<String> = lines.iter().map(|l| format!("[tr] {l}")).collect();
            if wrong_count {
                out.pop();
            }
            let reply = serde_json::json!({"candidates":[{"content":{"parts":[{"text": serde_json::to_string(&out).unwrap()}]}}]});
            (200, json, reply.to_string().into_bytes())
        }
        ("POST", "/azure/sts") => (200, "text/plain", b"fake-token".to_vec()),
        ("POST", "/azure/translate") => {
            let items: Vec<serde_json::Value> = serde_json::from_slice(body).unwrap_or_default();
            let to = path.split("to=").nth(1).and_then(|s| s.split('&').next()).unwrap_or("en");
            let mut out: Vec<serde_json::Value> = items
                .iter()
                .map(|i| serde_json::json!({"translations":[{"text": format!("[tr] {}", i["Text"].as_str().unwrap_or("")), "to": to}]}))
                .collect();
            if wrong_count {
                out.pop();
            }
            (200, json, serde_json::to_vec(&out).unwrap())
        }
        ("POST", "/azure/speak") => (200, "audio/x-wav", wav_24k(200)),
        ("GET", "/azure/voices") => (
            200,
            json,
            br#"[{"ShortName":"fr-FR-DeniseNeural","Gender":"Female","Locale":"fr-FR"},{"ShortName":"fr-FR-HenriNeural","Gender":"Male","Locale":"fr-FR"},{"ShortName":"en-US-JennyNeural","Gender":"Female","Locale":"en-US"},{"ShortName":"en-US-GuyNeural","Gender":"Male","Locale":"en-US"},{"ShortName":"km-KH-SreymomNeural","Gender":"Female","Locale":"km-KH"},{"ShortName":"km-KH-PisethNeural","Gender":"Male","Locale":"km-KH"}]"#.to_vec(),
        ),
        ("GET", "/azure/languages") => (
            200,
            json,
            br#"{"translation":{"en":{"name":"English"},"fr":{"name":"French"},"km":{"name":"Khmer"},"zh-Hans":{"name":"Chinese"},"pt":{"name":"Portuguese"}}}"#.to_vec(),
        ),
        _ => (404, json, br#"{"error":{"message":"no such fake endpoint"}}"#.to_vec()),
    }
}

fn respond(stream: &mut TcpStream, status: u16, extra: &str, ctype: &str, body: &[u8]) {
    let reason = match status {
        200 => "OK",
        401 => "Unauthorized",
        500 => "Internal Server Error",
        402 => "Payment Required",
        403 => "Forbidden",
        404 => "Not Found",
        429 => "Too Many Requests",
        _ => "Status",
    };
    let head = format!(
        "HTTP/1.1 {status} {reason}\r\n{extra}Content-Type: {ctype}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(body);
    let _ = stream.flush();
    let _ = stream.shutdown(Shutdown::Write);
}

type Parsed = (String, String, HashMap<String, String>, Vec<u8>);

fn read_request(stream: &mut TcpStream) -> Option<Parsed> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    let head_end = loop {
        let n = stream.read(&mut chunk).ok()?;
        if n == 0 {
            return None;
        }
        buf.extend_from_slice(&chunk[..n]);
        if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break i;
        }
    };
    let head = String::from_utf8_lossy(&buf[..head_end]).into_owned();
    let mut lines = head.split("\r\n");
    let mut first = lines.next()?.split(' ');
    let method = first.next()?.to_string();
    let path = first.next()?.to_string();
    let headers: HashMap<String, String> = lines
        .filter_map(|l| l.split_once(':'))
        .map(|(k, v)| (k.trim().to_ascii_lowercase(), v.trim().to_string()))
        .collect();
    let want: usize = headers
        .get("content-length")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let mut body = buf[head_end + 4..].to_vec();
    while body.len() < want {
        let n = stream.read(&mut chunk).ok()?;
        if n == 0 {
            break;
        }
        body.extend_from_slice(&chunk[..n]);
    }
    Some((method, path, headers, body))
}
