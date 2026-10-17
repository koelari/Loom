mod common;
use common::*;
use loom::{
    config,
    declaration::Declaration,
    resolver::{choose_version, resolve},
};
use std::{fs, os::unix::fs::symlink};

#[test]
fn numeric_selection_is_segment_based_and_overflow_free() {
    let versions: Vec<String> = [
        "21.0.9",
        "21.0.12",
        "210.0.0",
        "21.9999999999999999999999999999999999999",
        "nightly",
    ]
    .map(String::from)
    .into();
    assert_eq!(choose_version(&versions, "21.0"), Some("21.0.12".into()));
    assert_eq!(
        choose_version(&versions, "21"),
        Some("21.9999999999999999999999999999999999999".into())
    );
    assert_eq!(choose_version(&versions, "nightly"), Some("nightly".into()));
    assert_eq!(choose_version(&versions, "night"), None);
    assert_eq!(
        choose_version(&["21".into(), "21.0.12".into()], "21"),
        Some("21".into())
    );
}

#[test]
fn version_override_is_complete_and_does_not_inherit_variables() {
    let f = Fixture::new();
    f.tool("custom", "1.0", "[default]\npaths=['bin']\n[default.env]\nONLY_DEFAULT='yes'\n[versions.'1.0']\npaths=['.']\n");
    let set = resolve(
        &f.data,
        config::requests(&["custom@1".into()], &f.work).unwrap(),
    )
    .unwrap();
    assert!(set.variables.is_empty());
    assert!(set.paths[0].ends_with("custom/1.0"));
}

#[test]
fn invalid_override_never_falls_back() {
    let declaration: Declaration =
        toml::from_str("[default]\npaths=['bin']\n[versions.'1.0']\npaths=42").unwrap();
    assert!(declaration.select("1.0").is_err());
    assert!(declaration.select("2.0").is_ok());
}

#[test]
fn paths_cannot_escape_through_parent_or_symlink() {
    let f = Fixture::new();
    let root = f.tool("custom", "1", "[default]\npaths=['../other']");
    assert!(!f.loom(&["env", "custom@1"]).status.success());
    fs::write(
        root.parent().unwrap().join("loom.toml"),
        "[default]\npaths=['escape']",
    )
    .unwrap();
    symlink(&f.home, root.join("escape")).unwrap();
    let output = f.loom(&["env", "custom@1"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("escapes"));
    assert!(output.stdout.is_empty());
}

#[test]
fn shared_variables_merge_only_when_equal() {
    let f = Fixture::new();
    f.tool("one", "1", "[default.env]\nSHARED='same'");
    f.tool("two", "1", "[default.env]\nSHARED='different'");
    let output = f.loom(&["env", "one@1", "two@1"]);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    fs::write(
        f.data.join("tools/two/loom.toml"),
        "[default.env]\nSHARED='same'",
    )
    .unwrap();
    success(&f.loom(&["env", "one@1", "two@1"]));
}

#[test]
fn project_lookup_is_current_directory_only_and_explicit_paths_work() {
    let f = Fixture::new();
    f.tool("custom", "1", "[default]\npaths=['bin']");
    fs::write(f.work.join(".loom"), "custom='1'").unwrap();
    success(&f.loom(&["env"]));
    let subdir = f.work.join("subdir");
    fs::create_dir(&subdir).unwrap();
    let output = f
        .command(env!("CARGO_BIN_EXE_loom"))
        .current_dir(&subdir)
        .arg("env")
        .output()
        .unwrap();
    assert!(!output.status.success());
    let output = f
        .command(env!("CARGO_BIN_EXE_loom"))
        .current_dir(subdir)
        .args(["env", "../.loom"])
        .output()
        .unwrap();
    success(&output);
}

#[test]
fn unsafe_names_and_unknown_declaration_fields_are_rejected() {
    let f = Fixture::new();
    for request in ["../custom@1", "custom@..", "custom@1;evil", "custom@1@2"] {
        assert!(!f.loom(&["env", request]).status.success(), "{request}");
    }
    for declaration in [
        "[default.env]\nPATH='evil'",
        "[default.env]\nLOOM_STATE='evil'",
        "[default]\ncommand='evil'",
        "[default.env]\nBAD={path='.', extra=true}",
    ] {
        f.tool("custom", "1", declaration);
        let output = f.loom(&["env", "custom@1"]);
        assert!(!output.status.success(), "{declaration}");
        assert!(output.stdout.is_empty());
    }
}

#[test]
fn path_order_is_stable_and_duplicate_paths_are_removed() {
    let f = Fixture::new();
    f.tool("aaa", "1", "[default]\npaths=['bin','./bin','.']");
    f.tool("zzz", "1", "[default]\npaths=['bin']");
    let set = resolve(
        &f.data,
        config::requests(&["zzz@1".into(), "aaa@1".into()], &f.work).unwrap(),
    )
    .unwrap();
    assert_eq!(set.paths.len(), 3);
    assert!(set.paths[0].ends_with("aaa/1/bin"));
    assert!(set.paths[2].ends_with("zzz/1/bin"));
}
