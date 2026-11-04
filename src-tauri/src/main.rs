// Keep the console hidden in release builds on Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // Command-line mode used by the Windows installer: `flickfluent --check-hardware`.
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(out) = flickfluent_core::cli::run(&args) {
        print!("{}", out.stdout);
        std::process::exit(out.exit_code);
    }
    flickfluent_lib::run()
}
