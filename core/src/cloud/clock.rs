use std::future::Future;
use std::pin::Pin;
use std::sync::Mutex;
use std::time::Duration;

/// Time as the rotation code sees it, so tests can run ten minutes of waiting in no time.
pub trait Clock: Send + Sync {
    /// Milliseconds since an arbitrary start.
    fn now_ms(&self) -> u64;
}

pub trait Sleeper: Send + Sync {
    fn sleep<'a>(&'a self, seconds: u64) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>>;
}

pub struct RealClock {
    start: std::time::Instant,
}

impl RealClock {
    pub fn new() -> Self {
        Self {
            start: std::time::Instant::now(),
        }
    }
}

impl Default for RealClock {
    fn default() -> Self {
        Self::new()
    }
}

impl Clock for RealClock {
    fn now_ms(&self) -> u64 {
        self.start.elapsed().as_millis() as u64
    }
}

impl Sleeper for RealClock {
    fn sleep<'a>(&'a self, seconds: u64) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(tokio::time::sleep(Duration::from_secs(seconds)))
    }
}

/// Time that moves only when something sleeps. Records every sleep.
#[derive(Default)]
pub struct FakeClock {
    now_ms: Mutex<u64>,
    sleeps: Mutex<Vec<u64>>,
}

impl FakeClock {
    pub fn new() -> Self {
        Self::default()
    }

    /// Seconds slept, in order.
    pub fn sleeps(&self) -> Vec<u64> {
        self.sleeps.lock().unwrap().clone()
    }

    pub fn total_slept(&self) -> u64 {
        self.sleeps().iter().sum()
    }
}

impl Clock for FakeClock {
    fn now_ms(&self) -> u64 {
        *self.now_ms.lock().unwrap()
    }
}

impl Sleeper for FakeClock {
    fn sleep<'a>(&'a self, seconds: u64) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        self.sleeps.lock().unwrap().push(seconds);
        *self.now_ms.lock().unwrap() += seconds * 1000;
        Box::pin(std::future::ready(()))
    }
}
