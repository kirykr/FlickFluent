//! A benchmark run writes a record that follows `contracts/bench-record.schema.json`. Uses the
//! fake sidecar, so it tests the recording, not the models.

mod support;

use std::path::{Path, PathBuf};

use flickfluent_core::bench::{
    file_name, limited, load_records, run_bench, save_record, verdict, BenchOptions, BenchRecord,
    BenchResult, Limits,
};
use flickfluent_core::hardware::{Accelerator, MachineInput, OsInfo, SpecDefinition};
use flickfluent_core::project::dub::Stage;
use serde_json::json;
use support::run_fixture::config;

fn schema() -> serde_json::Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../specs/002-core-dub-pipeline/contracts/bench-record.schema.json");
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn clip() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../fixtures/clip-20s-fr.mp4")
}

fn machine() -> MachineInput {
    MachineInput {
        cpu_bits: Some(64),
        cpu_cores: Some(10),
        ram_gb: Some(16.0),
        free_disk_gb: Some(200.0),
        os: Some(OsInfo {
            family: "macos".into(),
            version: Some("26.0".into()),
        }),
        accelerator: Some(Accelerator::AppleSilicon),
    }
}

async fn record(limits: Option<Limits>) -> BenchRecord {
    let root = tempfile::tempdir().unwrap();
    let cfg = config(root.path(), "b.flick", json!({}));
    let opts = BenchOptions {
        fixture_id: "clip-20s-fr".into(),
        fixture_path: clip(),
        work_dir: root.path().join("bench-work"),
        simulated: limits,
        spec: SpecDefinition::bundled(),
    };
    let mut seen = Vec::new();
    let rec = run_bench(cfg, &machine(), &opts, &mut |s, _| seen.push(s))
        .await
        .unwrap();
    assert!(!seen.is_empty(), "progress was reported");
    rec
}

#[tokio::test]
async fn a_run_writes_a_record_that_validates() {
    let rec = record(None).await;
    let value = serde_json::to_value(&rec).unwrap();
    let validator = jsonschema::validator_for(&schema()).unwrap();
    let errors: Vec<String> = validator
        .iter_errors(&value)
        .map(|e| format!("{e} at {}", e.instance_path))
        .collect();
    assert!(errors.is_empty(), "{errors:?}\n{value:#}");

    assert!(!rec.machine.simulated);
    assert_eq!(rec.fixture.id, "clip-20s-fr");
    assert!(rec.fixture.duration_ms > 15_000);
    let stages: Vec<Stage> = rec.results.iter().map(|r| r.stage).collect();
    assert_eq!(
        stages,
        vec![
            Stage::Extract,
            Stage::Separate,
            Stage::Transcribe,
            Stage::Translate,
            Stage::Synthesize,
            Stage::Mix
        ]
    );
    // Worker stages name their model folder; the two core stages say ffmpeg.
    let model = |s: Stage| {
        rec.results
            .iter()
            .find(|r| r.stage == s)
            .unwrap()
            .model_id
            .clone()
    };
    assert_eq!(model(Stage::Transcribe), "whisper");
    assert_eq!(model(Stage::Translate), "m2m");
    assert_eq!(model(Stage::Extract), "ffmpeg");
    for r in &rec.results {
        let expected = r.wall_ms as f64 / rec.fixture.duration_ms as f64;
        assert!((r.real_time_factor - expected).abs() < 0.001, "{r:?}");
    }
    assert!(rec.results.iter().any(|r| r.peak_ram_mb.is_some()));
}

#[tokio::test]
async fn a_simulated_run_says_so_and_keeps_its_limits() {
    let limits = Limits {
        cpu_cores: 4,
        ram_gb: 8.0,
    };
    let rec = record(Some(limits)).await;
    assert!(rec.machine.simulated);
    assert_eq!(rec.machine.simulated_limits, Some(limits));
    // The record describes the machine the run stood for, not the one it ran on.
    assert_eq!(rec.machine.cores, Some(4));
    assert_eq!(rec.machine.ram_gb, Some(8.0));
    assert_eq!(rec.machine.accelerator, "none");
    assert!(!rec.verdict.can_run_full_local);
    let value = serde_json::to_value(&rec).unwrap();
    assert!(jsonschema::is_valid(&schema(), &value));
}

#[tokio::test]
async fn records_are_saved_one_file_each_and_read_back() {
    let rec = record(None).await;
    let dir = tempfile::tempdir().unwrap();
    let path = save_record(dir.path(), &rec).unwrap();
    assert_eq!(path.file_name().unwrap().to_string_lossy(), file_name(&rec));
    assert!(file_name(&rec).starts_with("bench-macos-") && file_name(&rec).contains("-real-"));
    std::fs::write(dir.path().join("notes.json"), "{}").unwrap();
    std::fs::write(dir.path().join("readme.txt"), "x").unwrap();
    assert_eq!(load_records(dir.path()), vec![rec]);
}

fn result(stage: Stage, peak: Option<u64>) -> BenchResult {
    BenchResult {
        stage,
        model_id: "m".into(),
        wall_ms: 1000,
        peak_ram_mb: peak,
        real_time_factor: 0.5,
    }
}

#[test]
fn the_verdict_needs_a_finished_run_and_memory_inside_the_share() {
    let spec = SpecDefinition::bundled();
    let full = limited(&machine(), None);
    let share = (spec.minimum.ram_gb * 1024.0 * 0.75) as u64;

    let fits = [result(Stage::Transcribe, Some(share))];
    let v = verdict(&fits, true, &full, &spec, 0.5);
    assert!(v.can_run_minimum, "exactly at the share still fits");

    let over = [result(Stage::Transcribe, Some(share + 1))];
    assert!(!verdict(&over, true, &full, &spec, 0.5).can_run_minimum);
    assert!(!verdict(&fits, false, &full, &spec, 0.5).can_run_minimum);
    // No memory figure at all proves nothing, and the notes say so.
    let unmeasured = verdict(&[result(Stage::Extract, None)], true, &full, &spec, 0.5);
    assert!(!unmeasured.can_run_minimum);
    assert!(unmeasured
        .notes
        .iter()
        .any(|n| n.contains("did not report their memory")));
}

#[test]
fn full_local_is_claimed_only_by_a_machine_that_meets_it() {
    let spec = SpecDefinition::bundled();
    let light = [result(Stage::Transcribe, Some(1000))];
    assert!(verdict(&light, true, &machine(), &spec, 0.5).can_run_full_local);
    let small = limited(
        &machine(),
        Some(Limits {
            cpu_cores: 4,
            ram_gb: 8.0,
        }),
    );
    let v = verdict(&light, true, &small, &spec, 0.5);
    assert!(v.can_run_minimum && !v.can_run_full_local);
    assert!(v
        .notes
        .iter()
        .any(|n| n.contains("does not meet the full-local")));
}
