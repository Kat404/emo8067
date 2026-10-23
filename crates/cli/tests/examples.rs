//! The eighty-five examples, run and pinned.
//!
//! The examples are the reference's own, and they are local gold: they are
//! not in the repository. The gate runs when they are there and stands
//! aside when they are not.
//!
//! The recorded state of every example lives beside them, in `goldens/`.
//! A run with no recorded state records it, and a run with one compares
//! against it and names the examples whose state changed. Delete the
//! `goldens/` directory to record them again.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Where the examples live, from this crate's directory.
fn examples() -> Option<PathBuf> {
    let path = Path::new("../../re/extracted/app/examples");
    if path.is_dir() {
        Some(path.to_path_buf())
    } else {
        None
    }
}

/// Where the reference's macro library lives, next to the examples.
fn library() -> Option<PathBuf> {
    let path = Path::new("../../re/extracted/app/inc");
    if path.is_dir() {
        Some(path.to_path_buf())
    } else {
        None
    }
}

/// Where the recorded states live, next to the examples.
fn goldens() -> PathBuf {
    Path::new("../../re/extracted/app/goldens").to_path_buf()
}

/// How the program stopped, as the golden says it did.
fn stop_of(golden: &str) -> String {
    if let Some(rest) = golden.split("STOP  took INT ").nth(1) {
        let vector: String = rest
            .trim_start_matches("0x")
            .chars()
            .take_while(|c| c.is_ascii_hexdigit())
            .collect();
        return format!("INT {vector}h");
    }
    if golden.contains("step budget exhausted") {
        return "the step budget".to_string();
    }
    "halted".to_string()
}

/// Run every example and say what each one left behind.
///
/// The name, the golden text, and how the program stopped.
fn run_examples(sources: &[PathBuf], library: &Path) -> Vec<(String, String, String)> {
    sources
        .iter()
        .map(|source| {
            let name = source.file_stem().unwrap().to_string_lossy().to_string();
            let output = Command::new(env!("CARGO_BIN_EXE_emo8067"))
                .args([
                    "run",
                    source.to_str().unwrap(),
                    "--include",
                    library.to_str().unwrap(),
                    "--golden",
                ])
                .output()
                .expect("the runner runs");
            let golden = String::from_utf8_lossy(&output.stdout).into_owned();
            let stop = stop_of(&golden);
            (name, golden, stop)
        })
        .collect()
}

#[test]
fn every_example_leaves_the_state_it_is_pinned_to() {
    let (Some(examples), Some(library)) = (examples(), library()) else {
        // The examples and the library are local gold, not part of the
        // repository.
        return;
    };
    let mut sources: Vec<_> = std::fs::read_dir(&examples)
        .expect("the examples")
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.extension().is_some_and(|extension| extension == "asm"))
        .collect();
    sources.sort();

    let goldens = goldens();
    std::fs::create_dir_all(&goldens).expect("the goldens' directory");

    // The examples are independent, so they run in parallel.
    let library = &library;
    let workers = std::thread::available_parallelism().map_or(1, |n| n.get());
    let chunk = sources.len().div_ceil(workers).max(1);
    let results: Vec<(String, String, String)> = std::thread::scope(|scope| {
        let handles: Vec<_> = sources
            .chunks(chunk)
            .map(|chunk| scope.spawn(move || run_examples(chunk, library)))
            .collect();
        handles
            .into_iter()
            .flat_map(|handle| handle.join().expect("the worker finishes"))
            .collect()
    });

    let mut matched = 0usize;
    let mut recorded = 0usize;
    let mut changed: Vec<String> = Vec::new();
    let mut stops: BTreeMap<String, usize> = BTreeMap::new();

    for (name, golden, stop) in results {
        *stops.entry(stop).or_default() += 1;
        let recorded_path = goldens.join(format!("{name}.txt"));
        if !recorded_path.exists() {
            std::fs::write(&recorded_path, &golden).expect("the state is recorded");
            recorded += 1;
            continue;
        }
        let expected = std::fs::read_to_string(&recorded_path).expect("the state is read");
        if expected == golden {
            matched += 1;
        } else {
            changed.push(name);
        }
    }

    println!(
        "{} examples, {matched} matched, {} changed, {recorded} recorded",
        sources.len(),
        changed.len()
    );
    for (stop, count) in &stops {
        println!("  {count} stopped at {stop}");
    }
    for name in &changed {
        println!("  {name} changed");
    }
    // A state that does not change when the core changes is the cheapest
    // regression harness there is.
    assert!(
        changed.is_empty(),
        "the examples whose state changed are: {changed:?}"
    );
}
