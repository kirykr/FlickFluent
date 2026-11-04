//! Command-line mode: `flickfluent --check-hardware [--message] [--fake-machine <file>]`.
//! Used by the Windows installer. Exit code 0 = fullLocal or hybrid, 10 = blocked, 2 = bad input.

use crate::hardware::{evaluate, probe, MachineInput, Mode, SpecDefinition};

pub const EXIT_BLOCKED: i32 = 10;

pub struct CliOutput {
    pub exit_code: i32,
    pub stdout: String,
}

/// Returns `None` when the arguments are not a CLI request, so the app should open its window.
pub fn run(args: &[String]) -> Option<CliOutput> {
    if !args.iter().any(|a| a == "--check-hardware") {
        return None;
    }
    let message_only = args.iter().any(|a| a == "--message");
    let fake = args
        .iter()
        .position(|a| a == "--fake-machine")
        .and_then(|i| args.get(i + 1));

    // Fake machines are for tests only; release builds always probe the real machine.
    let input = match fake {
        Some(path) if cfg!(debug_assertions) => match read_fake(path) {
            Ok(m) => m,
            Err(e) => {
                return Some(CliOutput {
                    exit_code: 2,
                    stdout: format!("{e}\n"),
                })
            }
        },
        _ => probe(),
    };

    let report = evaluate(&input, &SpecDefinition::bundled());
    let exit_code = if report.mode == Mode::Blocked {
        EXIT_BLOCKED
    } else {
        0
    };
    let stdout = if message_only {
        report
            .blocked_message
            .map(|m| format!("{m}\n"))
            .unwrap_or_default()
    } else {
        serde_json::to_string_pretty(&report).unwrap_or_default() + "\n"
    };
    Some(CliOutput { exit_code, stdout })
}

fn read_fake(path: &str) -> Result<MachineInput, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("cannot read {path}: {e}"))?;
    serde_json::from_str(&text).map_err(|e| format!("invalid fake machine file {path}: {e}"))
}
