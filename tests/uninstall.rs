mod common;
use common::*;
use std::{fs, path::Path};

fn uninstall(f: &Fixture) {
    let output = f
        .command("/bin/sh")
        .env("LOOM_BIN_DIR", &f.bin)
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/uninstall.sh"))
        .output()
        .unwrap();
    success(&output);
}

#[test]
fn uninstall_preserves_tools_and_unrelated_files_and_can_be_repeated() {
    let f = Fixture::new();
    f.tool("custom", "1", "[default]\npaths=['bin']");
    let shell = f.data.join("shell");
    fs::create_dir(&shell).unwrap();
    fs::write(shell.join("init.sh"), "integration").unwrap();
    fs::write(shell.join("user.sh"), "keep").unwrap();
    fs::write(f.bin.join("other"), "keep").unwrap();
    fs::write(f.home.join(".bashrc"), "user configuration").unwrap();
    uninstall(&f);
    assert!(!f.bin.join("loom").exists());
    assert!(!shell.join("init.sh").exists());
    assert_eq!(fs::read_to_string(shell.join("user.sh")).unwrap(), "keep");
    assert_eq!(fs::read_to_string(f.bin.join("other")).unwrap(), "keep");
    assert_eq!(
        fs::read_to_string(f.home.join(".bashrc")).unwrap(),
        "user configuration"
    );
    assert_eq!(
        fs::read_to_string(f.data.join("tools/custom/loom.toml")).unwrap(),
        "[default]\npaths=['bin']"
    );
    assert!(f.data.join("tools/custom/1/bin").is_dir());
    uninstall(&f);
}

#[test]
fn uninstall_removes_empty_integration_directory() {
    let f = Fixture::new();
    fs::create_dir(f.data.join("tools")).unwrap();
    let shell = f.data.join("shell");
    fs::create_dir(&shell).unwrap();
    fs::write(shell.join("init.sh"), "integration").unwrap();
    uninstall(&f);
    assert!(!shell.exists());
    assert!(f.data.exists());
}
