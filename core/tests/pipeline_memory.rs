//! A long file does not grow the memory of the sidecar or ffmpeg: each works on one section at a
//! time. Uses the fake sidecar for speed; the real long run is quickstart step 5.

mod support;

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use flickfluent_core::pipeline::run::{run_dub, CancelToken, Mode};
use serde_json::json;
use support::run_fixture::{config, dub_for, long_clip};
use sysinfo::{Pid, ProcessesToUpdate, System};

/// Largest resident memory, in MB, summed over all descendants of this process at one moment.
fn sample_children_mb(sys: &mut System, me: Pid) -> u64 {
    sys.refresh_processes(ProcessesToUpdate::All, true);
    let mut family = vec![me];
    let mut total = 0u64;
    let mut i = 0;
    while i < family.len() {
        let parent = family[i];
        for (pid, p) in sys.processes() {
            if p.parent() == Some(parent) && !family.contains(pid) {
                family.push(*pid);
                total += p.memory();
            }
        }
        i += 1;
    }
    total / (1024 * 1024)
}

#[tokio::test(flavor = "multi_thread")]
async fn sidecar_and_ffmpeg_memory_stays_under_the_ceiling_on_a_long_file() {
    const CEILING_MB: u64 = 600; // the minimum machine class for the stub; real ceilings are in the catalogue
    let root = tempfile::tempdir().unwrap();
    let source = long_clip(root.path(), 24);
    let cfg = config(root.path(), "m.flick", json!({}));
    let dub = dub_for(&source, "en").await;
    assert!(dub.source.as_ref().unwrap().duration_ms > 20 * 60 * 1000);

    let stop = Arc::new(AtomicBool::new(false));
    let peak = Arc::new(AtomicU64::new(0));
    let sampler = {
        let (stop, peak) = (stop.clone(), peak.clone());
        std::thread::spawn(move || {
            let mut sys = System::new();
            let me = Pid::from_u32(std::process::id());
            while !stop.load(Ordering::SeqCst) {
                peak.fetch_max(sample_children_mb(&mut sys, me), Ordering::SeqCst);
                std::thread::sleep(Duration::from_millis(50));
            }
        })
    };
    let out = run_dub(&cfg, dub, Mode::Fresh, CancelToken::default(), &mut |_| {})
        .await
        .unwrap();
    stop.store(true, Ordering::SeqCst);
    sampler.join().unwrap();

    assert!(
        out.run.unwrap().sections.len() >= 5,
        "a 24-minute file is several sections"
    );
    let peak = peak.load(Ordering::SeqCst);
    assert!(peak > 0, "the sampler saw the child processes");
    assert!(peak < CEILING_MB, "children peaked at {peak} MB");
}
