//! Several keys for one service: try them in the user's order, move on at a rate limit or a
//! lasting failure, wait when every key is limited, and give up (resumably) after ten minutes.
//! The rules are research R4; time is injected so tests need no real waiting.

use std::sync::Arc;

use std::future::Future;

use super::adapter::{CloudFailure, FailureKind};
use super::clock::{Clock, Sleeper};
use super::secret::Secret;

/// Longest the app waits on its own for one request, in seconds.
pub const MAX_WAIT_SECONDS: u64 = 600;
/// Wait used when a service does not say how long.
pub const DEFAULT_WAIT_SECONDS: u64 = 30;
/// While waiting on a limit, the pool reports at least this often.
pub const HEARTBEAT_SECONDS: u64 = 5;
/// Pause before the one retry of a server error on the same key.
const SERVER_RETRY_SECONDS: u64 = 2;

pub struct PoolKey {
    pub label: String,
    pub region: Option<String>,
    pub secret: Secret,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum RunState {
    Ready,
    /// Not before this time (milliseconds on the pool's clock).
    LimitedUntil(u64),
    SetAside(String),
}

/// What the user should hear about while a run goes on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PoolEvent {
    Switched { label: String },
    SetAside { label: String, reason: String },
    Waiting { seconds: u64 },
    Resumed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PoolFailure {
    /// Every key is limited and the waiting budget is used up. Resumable.
    Exhausted {
        retry_after_seconds: u64,
    },
    /// Every key was refused: `(label, reason)` for each.
    AuthFailed {
        keys: Vec<(String, String)>,
    },
    Offline {
        message: String,
    },
    /// The service answered something the app cannot use.
    BadReply {
        message: String,
    },
}

pub struct KeyPool {
    keys: Vec<PoolKey>,
    state: Vec<RunState>,
    clock: Arc<dyn Clock>,
    sleeper: Arc<dyn Sleeper>,
    last_used: Option<usize>,
    /// Counters for the usage record.
    pub switches: u64,
    pub waited_seconds: u64,
    pub keys_used: Vec<String>,
}

impl KeyPool {
    /// `keys` in the user's order. Every key starts the run as ready.
    pub fn new(keys: Vec<PoolKey>, clock: Arc<dyn Clock>, sleeper: Arc<dyn Sleeper>) -> Self {
        let state = vec![RunState::Ready; keys.len()];
        Self {
            keys,
            state,
            clock,
            sleeper,
            last_used: None,
            switches: 0,
            waited_seconds: 0,
            keys_used: Vec::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    fn refresh(&mut self) {
        let now = self.clock.now_ms();
        for s in &mut self.state {
            if matches!(s, RunState::LimitedUntil(t) if *t <= now) {
                *s = RunState::Ready;
            }
        }
    }

    fn first_ready(&self) -> Option<usize> {
        self.state.iter().position(|s| *s == RunState::Ready)
    }

    /// Run one request on the first usable key, moving on and waiting as the rules say. `f` gets
    /// a copy of the key and its region and returns the request; it may be called again for the
    /// next key. (A copy, because the request owns what it needs and the pool stays borrowable.)
    pub async fn call<T, Fut>(
        &mut self,
        mut f: impl FnMut(Secret, Option<String>) -> Fut,
        on_event: &mut (dyn FnMut(PoolEvent) + Send),
    ) -> Result<T, PoolFailure>
    where
        Fut: Future<Output = Result<T, CloudFailure>> + Send,
    {
        let mut waited = 0u64;
        // A key that already got its one retry after a server error during this request.
        let mut retried = vec![false; self.keys.len()];
        loop {
            self.refresh();
            let Some(i) = self.first_ready() else {
                let limited: Vec<u64> = self
                    .state
                    .iter()
                    .filter_map(|s| match s {
                        RunState::LimitedUntil(t) => Some(*t),
                        _ => None,
                    })
                    .collect();
                if limited.is_empty() {
                    return Err(PoolFailure::AuthFailed {
                        keys: self
                            .keys
                            .iter()
                            .zip(&self.state)
                            .filter_map(|(k, s)| match s {
                                RunState::SetAside(r) => Some((k.label.clone(), r.clone())),
                                _ => None,
                            })
                            .collect(),
                    });
                }
                // Every key left is limited: wait for the longest of them, then start over from
                // the first key.
                let now = self.clock.now_ms();
                let wait = limited
                    .iter()
                    .map(|t| t.saturating_sub(now))
                    .max()
                    .unwrap_or(0)
                    .div_ceil(1000)
                    .max(1);
                if waited + wait > MAX_WAIT_SECONDS {
                    return Err(PoolFailure::Exhausted {
                        retry_after_seconds: wait,
                    });
                }
                // Sleep in short steps and say so each time, so the screen keeps counting down.
                let mut left = wait;
                while left > 0 {
                    on_event(PoolEvent::Waiting { seconds: left });
                    let step = left.min(HEARTBEAT_SECONDS);
                    self.sleeper.sleep(step).await;
                    left -= step;
                }
                waited += wait;
                self.waited_seconds += wait;
                on_event(PoolEvent::Resumed);
                continue;
            };

            if self.last_used.is_some_and(|l| l != i) {
                self.switches += 1;
                on_event(PoolEvent::Switched {
                    label: self.keys[i].label.clone(),
                });
            }
            self.last_used = Some(i);
            if !self.keys_used.contains(&self.keys[i].label) {
                self.keys_used.push(self.keys[i].label.clone());
            }

            let result = f(self.keys[i].secret.clone(), self.keys[i].region.clone()).await;
            let failure: CloudFailure = match result {
                Ok(v) => return Ok(v),
                Err(f) => f,
            };
            let label = self.keys[i].label.clone();
            match failure.kind {
                FailureKind::RateLimited => self.limit(i, failure.retry_after),
                FailureKind::Unavailable if !retried[i] => {
                    retried[i] = true;
                    self.sleeper.sleep(SERVER_RETRY_SECONDS).await;
                }
                FailureKind::Unavailable => self.limit(i, None),
                FailureKind::Rejected | FailureKind::NoCredit => {
                    let reason = if failure.kind == FailureKind::Rejected {
                        "rejected"
                    } else {
                        "noCredit"
                    };
                    self.state[i] = RunState::SetAside(reason.to_string());
                    on_event(PoolEvent::SetAside {
                        label,
                        reason: reason.to_string(),
                    });
                }
                FailureKind::Offline => {
                    return Err(PoolFailure::Offline {
                        message: failure.message,
                    })
                }
                FailureKind::Unexpected => {
                    return Err(PoolFailure::BadReply {
                        message: failure.message,
                    })
                }
            }
        }
    }

    fn limit(&mut self, i: usize, retry_after: Option<u64>) {
        let secs = retry_after.unwrap_or(DEFAULT_WAIT_SECONDS);
        self.state[i] = RunState::LimitedUntil(self.clock.now_ms() + secs * 1000);
    }
}
