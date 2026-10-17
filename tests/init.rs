mod common;
use common::*;
use std::{
    fs,
    io::Write,
    os::unix::fs::symlink,
    process::{Output, Stdio},
};

fn interactive(f: &Fixture, input: &str) -> Output {
    let mut child = f
        .command(env!("CARGO_BIN_EXE_loom"))
        .arg("init")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn interactive_init_selects_multiple_tools_previews_and_writes_project() {
    let f = Fixture::new();
    f.tool("custom", "1.0", "[default]\npaths=['bin']");
    f.tool("custom", "2.0", "[default]\npaths=['bin']");
    f.tool("other", "3.0", "[default]\npaths=['bin']");
    // Invalid input reprompts, duplicate tool selections are deduplicated.
    let output = success(&interactive(&f, "invalid\n1,2,1\n\n1\n\n"));
    assert!(output.contains("Proposed .loom"));
    let file = fs::read_to_string(f.work.join(".loom")).unwrap();
    let parsed: toml::Table = toml::from_str(&file).unwrap();
    assert_eq!(parsed["custom"].as_str(), Some("2.0"));
    assert_eq!(parsed["other"].as_str(), Some("3.0"));
    success(&f.loom(&["env"]));
}

#[test]
fn cancelling_or_eof_does_not_create_configuration() {
    let f = Fixture::new();
    f.tool("custom", "1", "[default]\npaths=['bin']");
    for input in ["q\n", "", "1\nq\n", "1\n1\nn\n", "1\n1\n"] {
        assert!(success(&interactive(&f, input)).contains("cancelled"));
        assert!(!f.work.join(".loom").exists());
    }
}

#[test]
fn replacing_existing_configuration_requires_explicit_confirmation() {
    let f = Fixture::new();
    f.tool("custom", "1", "[default]\npaths=['bin']");
    fs::write(f.work.join(".loom"), "# keep this\ncustom='1'\n").unwrap();
    let output = success(&interactive(&f, "1\n1\n\n"));
    assert!(output.contains("Existing configuration"));
    assert!(output.contains("# keep this"));
    assert!(
        fs::read_to_string(f.work.join(".loom"))
            .unwrap()
            .starts_with("# keep")
    );
    success(&interactive(&f, "1\n1\ny\n"));
    assert!(
        !fs::read_to_string(f.work.join(".loom"))
            .unwrap()
            .contains("# keep")
    );
}

#[test]
fn explicit_init_preserves_selectors_and_force_controls_overwrite() {
    let f = Fixture::new();
    f.tool("custom", "1.2.3", "[default]\npaths=['bin']");
    success(&f.loom(&["init", "custom@1"]));
    let original = fs::read_to_string(f.work.join(".loom")).unwrap();
    assert!(original.contains("\"1\""));
    assert!(!f.loom(&["init", "custom@1.2.3"]).status.success());
    assert_eq!(fs::read_to_string(f.work.join(".loom")).unwrap(), original);
    success(&f.loom(&["init", "--force", "custom@1.2.3"]));
    assert!(
        fs::read_to_string(f.work.join(".loom"))
            .unwrap()
            .contains("1.2.3")
    );
}

#[test]
fn init_validation_errors_leave_existing_file_unchanged() {
    let f = Fixture::new();
    f.tool("custom", "1", "[default]\npaths=['missing']");
    fs::write(f.work.join(".loom"), "original").unwrap();
    assert!(!f.loom(&["init", "--force", "custom@1"]).status.success());
    assert_eq!(
        fs::read_to_string(f.work.join(".loom")).unwrap(),
        "original"
    );
}

#[test]
fn empty_or_invalid_local_inventory_is_reported() {
    let f = Fixture::new();
    assert!(!interactive(&f, "").status.success());
    f.tool("custom", "1", "[default]\npaths=42");
    assert!(!interactive(&f, "").status.success());
    assert!(!f.work.join(".loom").exists());
}

#[test]
fn init_refuses_symlink_or_directory_configuration() {
    let f = Fixture::new();
    f.tool("custom", "1", "[default]\npaths=['bin']");
    fs::write(f.work.join("original"), "keep").unwrap();
    symlink(f.work.join("original"), f.work.join(".loom")).unwrap();
    assert!(!f.loom(&["init", "--force", "custom@1"]).status.success());
    assert_eq!(fs::read_to_string(f.work.join("original")).unwrap(), "keep");
    fs::remove_file(f.work.join(".loom")).unwrap();
    fs::create_dir(f.work.join(".loom")).unwrap();
    assert!(!f.loom(&["init", "--force", "custom@1"]).status.success());
}
