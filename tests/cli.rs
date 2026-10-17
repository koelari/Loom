mod common;
use common::*;
use std::{fs, os::unix::fs::PermissionsExt};

#[test]
fn list_reads_directories_without_checking_sdk_paths() {
    let f = Fixture::new();
    f.tool("custom", "1", "[default]\npaths=['not-prepared-yet']");
    let output = success(&f.loom(&["list"]));
    assert!(output.contains("custom\n  1\n"));
    assert!(!output.contains("unavailable"));
    assert!(!f.loom(&["env", "custom@1"]).status.success());
    fs::remove_file(f.data.join("tools/custom/loom.toml")).unwrap();
    assert!(success(&f.loom(&["list"])).contains("unavailable"));
}

#[test]
fn which_obeys_path_and_ignores_nonexecutable_files_and_directories() {
    let f = Fixture::new();
    fs::write(f.bin.join("probe"), "not executable").unwrap();
    assert!(!f.loom(&["which", "probe"]).status.success());
    executable(&f.bin.join("probe"), "#!/bin/sh\nexit 0\n");
    assert_eq!(
        success(&f.loom(&["which", "probe"])).trim(),
        f.bin.join("probe").to_str().unwrap()
    );
    fs::set_permissions(f.bin.join("probe"), fs::Permissions::from_mode(0o601)).unwrap();
    // An owner without execute permission cannot execute merely because "other" can.
    assert!(!f.loom(&["which", "probe"]).status.success());
    fs::create_dir(f.bin.join("folder")).unwrap();
    assert!(!f.loom(&["which", "folder"]).status.success());
    assert!(!f.loom(&["which", "../probe"]).status.success());
    assert!(success(&f.loom(&["which", "sh"])).contains("/sh"));
}

#[test]
fn empty_state_commands_are_successful() {
    let f = Fixture::new();
    assert!(success(&f.loom(&["clear"])).is_empty());
    assert!(success(&f.loom(&["current"])).contains("No active"));
    assert!(success(&f.loom(&["list"])).contains("No local"));
    assert!(success(&f.loom(&["version"])).starts_with("loom 0.1."));
}

#[test]
fn corrupt_session_state_never_generates_shell_code() {
    let f = Fixture::new();
    let output = f.command(env!("CARGO_BIN_EXE_loom")).env("LOOM_STATE", r#"{"schema":1,"base_path":"x","originals":{"BAD;touch /tmp/oops":null},"tools":{},"source":"explicit"}"#).arg("clear").output().unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
}
