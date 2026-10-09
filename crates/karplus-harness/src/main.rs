//! `karplus-harness` — run the scenarios.
//!
//! ```text
//! karplus-harness run    [--scenarios DIR] [--filter TEXT] [--wav DIR]
//! karplus-harness check  [--scenarios DIR]
//! karplus-harness list   [--scenarios DIR]
//! ```
//!
//! `run` renders, measures and checks, and exits non-zero if anything fails.
//! `check` only parses. `--wav` writes every render as a 32-bit float WAV,
//! for listening.

use std::path::PathBuf;
use std::process::ExitCode;

use karplus_harness::run::{load, run};

struct Args {
    command: String,
    scenarios: PathBuf,
    filter: Option<String>,
    wav: Option<PathBuf>,
}

fn parse() -> Result<Args, String> {
    let mut args = std::env::args().skip(1);
    let command = args.next().ok_or("expected run, check or list")?;
    let mut parsed = Args {
        command,
        scenarios: PathBuf::from("scenarios"),
        filter: None,
        wav: None,
    };
    while let Some(flag) = args.next() {
        let mut value = || args.next().ok_or(format!("{flag} needs a value"));
        match flag.as_str() {
            "--scenarios" => parsed.scenarios = PathBuf::from(value()?),
            "--filter" => parsed.filter = Some(value()?),
            "--wav" => parsed.wav = Some(PathBuf::from(value()?)),
            other => return Err(format!("unknown flag {other}")),
        }
    }
    Ok(parsed)
}

fn main() -> ExitCode {
    let args = match parse() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("karplus-harness: {e}");
            return ExitCode::from(2);
        }
    };
    let scenarios = match load(&args.scenarios, args.filter.as_deref()) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("karplus-harness: {e}");
            return ExitCode::from(2);
        }
    };
    match args.command.as_str() {
        "list" => {
            for s in &scenarios {
                println!("{}", s.name);
            }
            ExitCode::SUCCESS
        }
        "check" => {
            println!("{}: {} parsed", args.scenarios.display(), scenarios.len());
            ExitCode::SUCCESS
        }
        "run" => {
            let mut failed = 0;
            for s in &scenarios {
                let outcome = run(s);
                let mark = if outcome.passed() { "ok  " } else { "FAIL" };
                println!("{mark} {}", outcome.name);
                for c in &outcome.checks {
                    let value = c
                        .measured
                        .map(|v| format!("{v:.3}"))
                        .unwrap_or_else(|| "not measurable".into());
                    let flag = if c.passed() { " " } else { "!" };
                    println!(
                        "   {flag} {:<20} {:>16}   ({})",
                        c.metric,
                        value,
                        c.bound.describe()
                    );
                }
                if let Some(dir) = &args.wav {
                    let path = dir.join(format!("{}.wav", s.name));
                    if let Err(e) = fathom_harness::wav::write(
                        &path,
                        &outcome.samples,
                        s.sample_rate,
                        fathom_harness::wav::Channels::DualMono,
                    ) {
                        eprintln!("karplus-harness: {}: {e}", path.display());
                        return ExitCode::from(2);
                    }
                }
                if !outcome.passed() {
                    failed += 1;
                }
            }
            println!();
            println!("{} scenarios, {failed} failed", scenarios.len());
            if failed > 0 {
                ExitCode::FAILURE
            } else {
                ExitCode::SUCCESS
            }
        }
        other => {
            eprintln!("karplus-harness: unknown command {other}");
            ExitCode::from(2)
        }
    }
}
