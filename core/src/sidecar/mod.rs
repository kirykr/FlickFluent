//! The Python sidecar: start it, talk JSON lines, cancel, and recover from a crash.
//! See `specs/002-core-dub-pipeline/contracts/sidecar-protocol.md`.

use std::collections::VecDeque;
use std::ffi::OsString;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::Deserialize;
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::time::timeout;

use crate::error::{
    AppError, CANCELLED, NO_SPACE, PATH_NOT_ALLOWED, SIDECAR_CRASHED, SIDECAR_START_FAILED,
    STAGE_FAILED,
};

pub const PROTOCOL: u64 = 1;
const STDERR_TAIL: usize = 50;
/// How often a silent request wakes the caller with `Event::Tick`.
const TICK: Duration = Duration::from_secs(2);

#[derive(Debug, Clone)]
pub struct SidecarConfig {
    pub program: PathBuf,
    pub args: Vec<OsString>,
    /// No line from the sidecar for this long while a request runs counts as a hang.
    pub hang_timeout: Duration,
    pub start_timeout: Duration,
}

#[derive(Debug, Clone)]
pub struct Request {
    pub worker: String,
    pub input: Value,
    pub output: Value,
    pub models: Value,
    pub params: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    Progress(f32),
    Item {
        kind: String,
        data: Value,
    },
    /// The worker said nothing for a while but is not hung yet. Lets the caller send a heartbeat.
    Tick,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Stats {
    #[serde(default)]
    pub wall_ms: u64,
    #[serde(default)]
    pub peak_ram_mb: u64,
}

#[derive(Debug, Clone)]
pub struct Done {
    pub ok: bool,
    pub result: Value,
    pub files: Vec<String>,
    pub stats: Stats,
}

/// Why a request did not finish. Only `Worker` is the sidecar's own answer.
enum Failure {
    Died,
    Hung,
    Worker(AppError),
}

pub struct Sidecar {
    cfg: SidecarConfig,
    child: Child,
    stdin: Arc<tokio::sync::Mutex<ChildStdin>>,
    lines: Lines<BufReader<ChildStdout>>,
    workers: Vec<String>,
    next_id: u64,
    current: Arc<Mutex<Option<String>>>,
    restart_pending: bool,
    stderr: Arc<Mutex<VecDeque<String>>>,
}

/// Lets another task stop the request that is running.
#[derive(Clone)]
pub struct CancelHandle {
    stdin: Arc<tokio::sync::Mutex<ChildStdin>>,
    current: Arc<Mutex<Option<String>>>,
}

impl CancelHandle {
    pub async fn cancel(&self) {
        let target = self.current.lock().ok().and_then(|c| c.clone());
        if let Some(target) = target {
            let line = json!({ "cmd": "cancel", "target": target }).to_string() + "\n";
            let mut stdin = self.stdin.lock().await;
            let _ = stdin.write_all(line.as_bytes()).await;
            let _ = stdin.flush().await;
        }
    }
}

fn start_failed(detail: impl Into<String>) -> AppError {
    AppError::new(SIDECAR_START_FAILED, "dub.error.sidecarStart")
        .with_details(json!({ "detail": detail.into() }))
}

fn crashed(tail: &Arc<Mutex<VecDeque<String>>>) -> AppError {
    let tail: Vec<String> = tail
        .lock()
        .map(|t| t.iter().cloned().collect())
        .unwrap_or_default();
    AppError::new(SIDECAR_CRASHED, "dub.error.sidecarCrashed")
        .with_details(json!({ "stderrTail": tail }))
}

/// Error codes the sidecar may send that the UI knows by name. Anything else is a stage failure.
fn worker_error(err: &Value) -> AppError {
    let code = err.get("code").and_then(Value::as_str).unwrap_or("");
    let detail = json!({
        "workerCode": code,
        "message": err.get("message").cloned().unwrap_or(Value::Null),
        "retryable": err.get("retryable").cloned().unwrap_or(Value::Null),
    });
    match code {
        "CANCELLED" => AppError::new(CANCELLED, "dub.error.cancelled"),
        "NO_SPACE" => AppError::new(NO_SPACE, "dub.error.noSpace").with_details(detail),
        "PATH_NOT_ALLOWED" => {
            AppError::new(PATH_NOT_ALLOWED, "dub.error.pathNotAllowed").with_details(detail)
        }
        _ => AppError::new(STAGE_FAILED, "dub.error.stageFailed").with_details(detail),
    }
}

impl Sidecar {
    pub async fn start(cfg: SidecarConfig) -> Result<Self, AppError> {
        let mut child = Command::new(&cfg.program)
            .args(&cfg.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| start_failed(format!("{}: {e}", cfg.program.display())))?;
        let stdin = child.stdin.take().ok_or_else(|| start_failed("no stdin"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| start_failed("no stdout"))?;
        let stderr_pipe = child
            .stderr
            .take()
            .ok_or_else(|| start_failed("no stderr"))?;

        let stderr = Arc::new(Mutex::new(VecDeque::new()));
        let tail = stderr.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stderr_pipe).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if let Ok(mut t) = tail.lock() {
                    if t.len() == STDERR_TAIL {
                        t.pop_front();
                    }
                    t.push_back(line);
                }
            }
        });

        let mut sc = Self {
            cfg,
            child,
            stdin: Arc::new(tokio::sync::Mutex::new(stdin)),
            lines: BufReader::new(stdout).lines(),
            workers: Vec::new(),
            next_id: 0,
            current: Arc::new(Mutex::new(None)),
            restart_pending: false,
            stderr,
        };
        let limit = sc.cfg.start_timeout;
        match timeout(limit, sc.handshake()).await {
            Ok(Ok(())) => Ok(sc),
            Ok(Err(e)) => Err(e),
            Err(_) => Err(start_failed("the sidecar did not become ready in time")),
        }
    }

    async fn handshake(&mut self) -> Result<(), AppError> {
        loop {
            let line = self
                .lines
                .next_line()
                .await
                .map_err(|e| start_failed(e.to_string()))?
                .ok_or_else(|| start_failed("the sidecar exited before it was ready"))?;
            let Ok(msg) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            if msg["event"] == "ready" {
                self.workers = msg["workers"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .filter_map(|w| w.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();
                break;
            }
        }
        self.send(&json!({ "id": "hello", "cmd": "hello", "protocol": PROTOCOL }))
            .await
            .map_err(|_| start_failed("could not write to the sidecar"))?;
        loop {
            let line = self
                .lines
                .next_line()
                .await
                .map_err(|e| start_failed(e.to_string()))?
                .ok_or_else(|| start_failed("the sidecar exited during the handshake"))?;
            let Ok(msg) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            if msg["id"] == "hello" {
                return if msg["ok"] == true {
                    Ok(())
                } else {
                    Err(start_failed("protocol version mismatch"))
                };
            }
        }
    }

    pub fn workers(&self) -> &[String] {
        &self.workers
    }

    pub fn cancel_handle(&self) -> CancelHandle {
        CancelHandle {
            stdin: self.stdin.clone(),
            current: self.current.clone(),
        }
    }

    async fn send(&self, msg: &Value) -> std::io::Result<()> {
        let mut stdin = self.stdin.lock().await;
        stdin.write_all((msg.to_string() + "\n").as_bytes()).await?;
        stdin.flush().await
    }

    async fn run_inner(
        &mut self,
        req: &Request,
        on_event: &mut (dyn FnMut(Event) + Send),
    ) -> Result<Done, Failure> {
        self.next_id += 1;
        let id = format!("r{}", self.next_id);
        if let Ok(mut c) = self.current.lock() {
            *c = Some(id.clone());
        }
        let msg = json!({
            "id": id, "cmd": "run", "worker": req.worker,
            "input": req.input, "output": req.output, "models": req.models, "params": req.params,
        });
        let outcome = self.read_until_done(&id, msg, on_event).await;
        if let Ok(mut c) = self.current.lock() {
            *c = None;
        }
        outcome
    }

    async fn read_until_done(
        &mut self,
        id: &str,
        msg: Value,
        on_event: &mut (dyn FnMut(Event) + Send),
    ) -> Result<Done, Failure> {
        self.send(&msg).await.map_err(|_| Failure::Died)?;
        let slice = TICK.min(self.cfg.hang_timeout);
        let mut idle = Duration::ZERO;
        loop {
            let next = timeout(slice, self.lines.next_line()).await;
            let line = match next {
                Err(_) => {
                    idle += slice;
                    if idle >= self.cfg.hang_timeout {
                        return Err(Failure::Hung);
                    }
                    on_event(Event::Tick);
                    continue;
                }
                Ok(Err(_)) | Ok(Ok(None)) => return Err(Failure::Died),
                Ok(Ok(Some(l))) => l,
            };
            idle = Duration::ZERO;
            let Ok(m) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            if m["id"] != id {
                continue;
            }
            match m["event"].as_str() {
                Some("progress") => {
                    on_event(Event::Progress(m["progress"].as_f64().unwrap_or(0.0) as f32));
                    continue;
                }
                Some("item") => {
                    on_event(Event::Item {
                        kind: m["kind"].as_str().unwrap_or("").to_string(),
                        data: m["data"].clone(),
                    });
                    continue;
                }
                Some("restart_requested") => {
                    self.restart_pending = true;
                    continue;
                }
                _ => {}
            }
            if m["ok"] == true {
                return Ok(Done {
                    ok: true,
                    result: m["result"].clone(),
                    files: m["files"]
                        .as_array()
                        .map(|a| {
                            a.iter()
                                .filter_map(|f| f.as_str().map(String::from))
                                .collect()
                        })
                        .unwrap_or_default(),
                    stats: serde_json::from_value(m["stats"].clone()).unwrap_or_default(),
                });
            }
            if m["ok"] == false {
                return Err(Failure::Worker(worker_error(&m["error"])));
            }
        }
    }

    /// Run one request. A dead or silent sidecar gives `SIDECAR_CRASHED` and is not retried.
    pub async fn run(
        &mut self,
        req: Request,
        on_event: &mut (dyn FnMut(Event) + Send),
    ) -> Result<Done, AppError> {
        if self.restart_pending {
            self.restart().await?;
        }
        match self.run_inner(&req, on_event).await {
            Ok(d) => Ok(d),
            Err(Failure::Worker(e)) => Err(e),
            Err(_) => Err(crashed(&self.stderr)),
        }
    }

    /// Like `run`, but a crash or hang restarts the sidecar and tries once more. A second
    /// failure gives `SIDECAR_CRASHED`.
    pub async fn run_with_retry(
        &mut self,
        req: Request,
        on_event: &mut (dyn FnMut(Event) + Send),
    ) -> Result<Done, AppError> {
        if self.restart_pending {
            self.restart().await?;
        }
        match self.run_inner(&req, on_event).await {
            Ok(d) => return Ok(d),
            Err(Failure::Worker(e)) => return Err(e),
            Err(_) => {}
        }
        self.restart().await?;
        match self.run_inner(&req, on_event).await {
            Ok(d) => Ok(d),
            Err(Failure::Worker(e)) => Err(e),
            Err(_) => Err(crashed(&self.stderr)),
        }
    }

    /// Kill the process and start a fresh one with the same settings.
    pub async fn restart(&mut self) -> Result<(), AppError> {
        let _ = self.child.kill().await;
        let fresh = Sidecar::start(self.cfg.clone()).await?;
        *self = fresh;
        Ok(())
    }

    pub async fn shutdown(&mut self) {
        let _ = self.send(&json!({ "id": "bye", "cmd": "shutdown" })).await;
        if timeout(Duration::from_secs(3), self.child.wait())
            .await
            .is_err()
        {
            let _ = self.child.kill().await;
        }
    }
}
