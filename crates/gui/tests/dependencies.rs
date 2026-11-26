//! The GUI's own dependencies: the rule the crate was made under.
//!
//! The GUI is the core's consumer and never its dependency, and the core
//! and the assembler keep their empty dependency lists: "the core never
//! formats a string, never opens a socket, and never knows a window exists"
//! (`odd/research/features-and-ux.md:604`-`:607`).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// One dependency, as the manifest writes it.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Entry {
    /// The table it is declared in.
    table: String,
    /// The declaration itself, with the lines of a long value joined.
    text: String,
}

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

/// Every dependency a manifest declares, in every table.
///
/// A dependency is a key of a table whose last name is `dependencies`:
/// `[dependencies]`, `[dev-dependencies]` and
/// `[target.'cfg(unix)'.dependencies]` are all such tables, and a
/// dependency declared for one target only is a dependency all the same. A
/// dependency written as a table of its own — `[dependencies.foo]` — is the
/// dependency `foo`.
///
/// The manifest is read as text and not parsed: the crate takes no parser
/// for it, and the rule is about what is written down.
fn dependencies(manifest: &str) -> BTreeMap<String, Entry> {
    let mut out: BTreeMap<String, Entry> = BTreeMap::new();
    let mut table = String::new();
    let mut own: Option<String> = None;
    let mut own_text = String::new();
    let mut pending: Option<String> = None;
    let mut pending_text = String::new();
    let mut open = 0i32;

    for raw in manifest.lines() {
        let line = raw.trim();

        // A value that runs over more than one line.
        if let Some(name) = pending.clone() {
            pending_text.push(' ');
            pending_text.push_str(line);
            open += balance(line);
            if open <= 0 {
                out.insert(
                    name,
                    Entry {
                        table: table.clone(),
                        text: pending_text.clone(),
                    },
                );
                pending = None;
                pending_text.clear();
            }
            continue;
        }

        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if line.starts_with('[') {
            table = line
                .trim_start_matches('[')
                .trim_end_matches(']')
                .trim()
                .to_string();
            match own_dependency(&table) {
                Some(name) => {
                    own = Some(name);
                    own_text.clear();
                }
                None => own = None,
            }
            continue;
        }

        // A dependency written as a table of its own: its body is the
        // declaration.
        if let Some(name) = own.clone() {
            own_text.push(' ');
            own_text.push_str(line);
            out.insert(
                name,
                Entry {
                    table: parent_table(&table),
                    text: own_text.clone(),
                },
            );
            continue;
        }

        if !is_dependency_table(&table) {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let name = key.trim().to_string();
        pending_text = value.trim().to_string();
        open = balance(&pending_text);
        if open <= 0 {
            out.insert(
                name,
                Entry {
                    table: table.clone(),
                    text: pending_text.clone(),
                },
            );
            pending_text.clear();
        } else {
            pending = Some(name);
        }
    }
    out
}

/// Whether a table holds dependencies.
fn is_dependency_table(table: &str) -> bool {
    let last = table.rsplit('.').next().unwrap_or_default();
    last == "dependencies" || last.ends_with("-dependencies")
}

/// The dependency a table of its own declares.
fn own_dependency(table: &str) -> Option<String> {
    let mut parts = table.rsplit('.');
    let name = parts.next()?;
    let parent = parts.next()?;
    if parent == "dependencies" || parent.ends_with("-dependencies") {
        Some(name.to_string())
    } else {
        None
    }
}

/// The table a dependency of its own belongs to.
fn parent_table(table: &str) -> String {
    match table.rfind('.') {
        Some(at) => table[..at].to_string(),
        None => String::new(),
    }
}

/// The brackets a line opens and closes.
fn balance(line: &str) -> i32 {
    let mut open = 0;
    for character in line.chars() {
        match character {
            '[' | '{' => open += 1,
            ']' | '}' => open -= 1,
            _ => {}
        }
    }
    open
}

#[test]
fn the_core_and_the_assembler_keep_their_empty_dependency_lists() {
    for crate_name in ["core", "asm"] {
        let dependencies = dependencies(&manifest(crate_name));
        assert!(
            dependencies.is_empty(),
            "{crate_name} depends on nothing, in any table: the core never formats a string, \
             never opens a socket, and never knows a window exists"
        );
    }
}

#[test]
fn the_gui_depends_on_the_core_the_debug_and_the_asm_and_on_nothing_else_of_the_projects_own() {
    let own: Vec<String> = dependencies(&manifest("gui"))
        .keys()
        .filter_map(|name| name.strip_prefix("emo8067-"))
        .map(String::from)
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
    let dependencies = dependencies(&manifest("gui"));

    let eframe = dependencies.get("eframe").expect("the GUI takes eframe");
    assert_eq!(
        eframe.table, "dependencies",
        "eframe is a runtime dependency"
    );
    assert!(
        eframe.text.contains("0.36.2"),
        "eframe 0.36.2: {}",
        eframe.text
    );
    assert!(
        eframe.text.contains("default-features = false"),
        "eframe's default features are off: {}",
        eframe.text
    );
    assert!(
        !eframe.text.contains("accesskit"),
        "accesskit stays out: the accessibility story this project has is the TUI's: {}",
        eframe.text
    );

    let egui = dependencies.get("egui").expect("the GUI takes egui");
    assert_eq!(egui.table, "dependencies", "egui is a runtime dependency");
    assert!(egui.text.contains("0.36.2"), "egui 0.36.2: {}", egui.text);
}

#[test]
fn the_scanner_sees_every_table_and_every_line_of_a_value() {
    let manifest = "\
[package]
name = \"a\"

[dependencies]
one = \"1.0\"
two = { version = \"2.0\",
        features = [\"x\"] }

[dev-dependencies]
three = \"3.0\"

[target.'cfg(unix)'.dependencies]
four = \"4.0\"

[build-dependencies]
five = { version = \"5.0\" }

[dependencies.six]
version = \"6.0\"
";

    let dependencies = dependencies(manifest);

    let names: Vec<&str> = dependencies.keys().map(String::as_str).collect();
    assert_eq!(
        names,
        ["five", "four", "one", "six", "three", "two"],
        "every table is seen, and the set is a set"
    );
    assert_eq!(
        dependencies["two"].text, "{ version = \"2.0\", features = [\"x\"] }",
        "a value that runs over lines is one entry and not three"
    );
    assert_eq!(
        dependencies["four"].table, "target.'cfg(unix)'.dependencies",
        "and a dependency declared for one target only is a dependency"
    );
    assert_eq!(
        dependencies["six"].table, "dependencies",
        "a dependency written as a table of its own is in that table"
    );
}
