//! QA runner for isonomic-ui component stories.
//!
//!   cargo run -p qa-runner -- run card
//!   cargo run -p qa-runner -- run card --step 3
//!   cargo run -p qa-runner -- run --all
//!   cargo run -p qa-runner -- list
//!
//! Builds nothing itself: it checks that the QA gallery is built, then hands
//! over to `runner/driver/qa_driver.py`, which drives the gallery under a
//! virtual display and writes `qa/out/<slug>/results.json`. The runner and the
//! gallery live only in the `qa/` workspace and are never part of a release
//! build.

use std::{
    env,
    path::PathBuf,
    process::{Command, ExitCode},
};

fn main() -> ExitCode {
    let qa = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("runner sits inside qa/")
        .to_path_buf();
    let args: Vec<String> = env::args().skip(1).collect();

    if args.first().map(String::as_str) == Some("run") {
        // Make sure the gallery matches the sources before driving it.
        let status = Command::new(env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
            .args(["build", "-p", "qa-gallery"])
            .current_dir(&qa)
            .status();
        match status {
            Ok(status) if status.success() => {}
            Ok(_) => {
                eprintln!("qa-runner: the QA gallery does not build; that is a C1 failure for every story");
                return ExitCode::from(2);
            }
            Err(err) => {
                eprintln!("qa-runner: could not run cargo: {err}");
                return ExitCode::from(2);
            }
        }
    }

    let python = env::var("QA_PYTHON").unwrap_or_else(|_| "/usr/bin/python3.12".into());
    let driver = qa.join("runner/driver/qa_driver.py");
    match Command::new(&python).arg("-I").arg(&driver).args(&args).status() {
        Ok(status) => ExitCode::from(status.code().unwrap_or(1) as u8),
        Err(err) => {
            eprintln!("qa-runner: could not start {python}: {err}");
            ExitCode::from(2)
        }
    }
}
