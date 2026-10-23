#[path = "support/household_wage_spatial.rs"]
mod scenario;

use std::{env, path::PathBuf, process::ExitCode};

fn main() -> ExitCode {
    let mut args = env::args_os().skip(1);
    let Some(first) = args.next() else {
        eprintln!(
            "usage: town_household_wage_spatial <new-artifact-root>\n\
             town_household_wage_spatial --verify-open <existing-world>\n\
             Output is never written over an existing path."
        );
        return ExitCode::from(2);
    };
    if first.to_str() == Some("--verify-open") {
        let Some(source) = args.next().map(PathBuf::from) else {
            eprintln!("usage: town_household_wage_spatial --verify-open <existing-world>");
            return ExitCode::from(2);
        };
        if args.next().is_some() {
            eprintln!("unexpected extra argument to --verify-open");
            return ExitCode::from(2);
        }
        return match scenario::verify_open(&source) {
            Ok(report) => {
                println!("HOUSEHOLD_WAGE_SPATIAL_VERIFY_OPEN={report}");
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("HOUSEHOLD_WAGE_SPATIAL_VERIFY_OPEN_FAILED={error}");
                ExitCode::FAILURE
            }
        };
    }
    let path = PathBuf::from(first);
    if args.next().is_some() {
        eprintln!("unexpected extra argument");
        return ExitCode::from(2);
    }
    if path.exists() {
        eprintln!("refusing existing artifact path: {}", path.display());
        return ExitCode::from(2);
    }
    if let Err(error) = std::fs::create_dir_all(&path) {
        eprintln!("cannot create artifact path {}: {error}", path.display());
        return ExitCode::FAILURE;
    }
    let path = match std::fs::canonicalize(path) {
        Ok(path) => path,
        Err(error) => {
            eprintln!("cannot resolve artifact root: {error}");
            return ExitCode::FAILURE;
        }
    };
    let report = scenario::run_pair(&path);
    println!(
        "HOUSEHOLD_WAGE_SPATIAL_RESULT={}",
        scenario::report_json(&report)
    );
    println!("ARTIFACT_ROOT={}", path.display());
    ExitCode::SUCCESS
}
