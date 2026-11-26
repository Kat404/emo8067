//! The GUI's own dependencies: the rule the crate was made under.
//!
//! The GUI is the core's consumer and never its dependency, and the core
//! and the assembler keep their empty dependency lists: "the core never
//! formats a string, never opens a socket, and never knows a window exists"
//! (`odd/research/features-and-ux.md:604`-`:607`).

use std::path::{Path, PathBuf};

/// The workspace's own directory.
fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("the crate is inside the workspace")
        .to_path_buf()
}

/// The manifest of one of the workspace's crates.
fn manifest(crate_name: &str) -> String {
    let path = workspace()
        .join("crates")
        .join(crate_name)
        .join("Cargo.toml");
    std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

/// The entries of one table of a manifest.
///
/// The manifests are read as text and not as a parsed document: the rule
/// is about what is written down, and the tree takes no parser for it.
fn table(manifest: &str, name: &str) -> Vec<String> {
    let mut entries = Vec::new();
    let mut inside = false;
    for line in manifest.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            inside = line == format!("[{name}]");
            continue;
        }
        if inside && !line.is_empty() && !line.starts_with('#') {
            entries.push(line.to_string());
        }
    }
    entries
}

/// The GUI's own dependencies.
fn dependencies() -> Vec<String> {
    table(&manifest("gui"), "dependencies")
}

#[test]
fn the_core_and_the_assembler_keep_their_empty_dependency_lists() {
    for crate_name in ["core", "asm"] {
        let manifest = manifest(crate_name);
        assert!(
            manifest.contains("[dependencies]"),
            "{crate_name} declares a dependency table"
        );
        assert_eq!(
            table(&manifest, "dependencies"),
            Vec::<String>::new(),
            "{crate_name} depends on nothing: the core never formats a string, never opens a \
             socket, and never knows a window exists"
        );
    }
}

#[test]
fn the_gui_depends_on_the_core_the_debug_and_the_asm_and_on_nothing_else_of_the_projects_own() {
    let own: Vec<String> = dependencies()
        .iter()
        .filter_map(|entry| entry.strip_prefix("emo8067-"))
        .map(|entry| {
            entry
                .split([' ', '='])
                .next()
                .unwrap_or_default()
                .to_string()
        })
        .collect();
    assert_eq!(
        own,
        ["asm", "core", "debug"],
        "the GUI takes the core, the debugger's core and the assembler, and nothing else of the \
         project's own"
    );
}

#[test]
fn the_gui_takes_egui_and_eframe_without_eframe_s_defaults() {
    let dependencies = dependencies();
    let eframe = dependencies
        .iter()
        .find(|entry| entry.starts_with("eframe"))
        .expect("the GUI takes eframe");
    assert!(eframe.contains("0.36.2"), "eframe 0.36.2: {eframe}");
    assert!(
        eframe.contains("default-features = false"),
        "eframe's default features are off: {eframe}"
    );
    assert!(
        !eframe.contains("accesskit"),
        "accesskit stays out: the accessibility story this project has is the TUI's: {eframe}"
    );
    let egui = dependencies
        .iter()
        .find(|entry| entry.starts_with("egui "))
        .expect("the GUI takes egui");
    assert!(egui.contains("0.36.2"), "egui 0.36.2: {egui}");
}

#[test]
fn the_workspace_floor_is_the_one_egui_declares() {
    let manifest = std::fs::read_to_string(workspace().join("Cargo.toml"))
        .expect("the workspace's own manifest");
    assert!(
        manifest.contains("rust-version = \"1.95\""),
        "the floor is the one egui 0.36.2 declares"
    );
}
