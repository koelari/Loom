mod common;
use common::*;
use std::fs;

const SHELLS: [&str; 2] = ["/bin/bash", "/bin/zsh"];

#[test]
fn repeated_activation_replaces_collection_and_clear_restores_originals() {
    let f = Fixture::new();
    f.tool(
        "one",
        "1",
        "[default]\npaths=['bin']\n[default.env]\nORIGINAL='changed'\nNEW_VAR='new'",
    );
    f.tool(
        "two",
        "2",
        "[default]\npaths=['bin']\n[default.env]\nSECOND='yes'",
    );
    for shell in SHELLS {
        success(&f.shell(
            shell,
            r#"
set -e
. "$INIT"
. "$INIT"
export ORIGINAL='before'
BASE=$PATH
loom clear
loom use one@1
test "$ORIGINAL" = changed
test "$NEW_VAR" = new
FIRST=$PATH
loom use one@1
test "$PATH" = "$FIRST"
loom use two@2
test "$ORIGINAL" = before
test "${NEW_VAR+x}" != x
test "$SECOND" = yes
case "$PATH" in *one/1/bin*) exit 30;; esac
loom current | /usr/bin/grep 'two  2'
loom clear
test "$PATH" = "$BASE"
test "$ORIGINAL" = before
test "${SECOND+x}" != x
test "${LOOM_STATE+x}" != x
loom clear
"#,
        ));
    }
}

#[test]
fn failed_activation_leaves_active_environment_untouched() {
    let f = Fixture::new();
    f.tool(
        "one",
        "1",
        "[default]\npaths=['bin']\n[default.env]\nSHARED='first'",
    );
    f.tool("two", "1", "[default.env]\nSHARED='second'");
    for shell in SHELLS {
        success(&f.shell(
            shell,
            r#"
set -e
. "$INIT"
loom use one@1
BEFORE_PATH=$PATH
BEFORE_STATE=$LOOM_STATE
if loom use one@1 missing@1; then exit 31; fi
if loom use one@1 two@1; then exit 32; fi
test "$PATH" = "$BEFORE_PATH"
test "$LOOM_STATE" = "$BEFORE_STATE"
test "$SHARED" = first
"#,
        ));
    }
}

#[test]
fn literal_values_are_quoted_without_execution() {
    let f = Fixture::new();
    f.tool("custom", "1", "[default]\npaths=['bin']\n[default.env]\nPAYLOAD=\"hello ' \\\" $(touch INJECTED) `touch ALSO_INJECTED`\\nnext\"\nTOOL_HOME={path='.'}");
    for shell in SHELLS {
        let output = f.shell(
            shell,
            r#"
set -e
. "$INIT"
loom use custom@1
printf '%s' "$PAYLOAD"
test -d "$TOOL_HOME"
loom clear
"#,
        );
        assert!(success(&output).contains("$(touch INJECTED)"));
        assert!(!f.work.join("INJECTED").exists());
        assert!(!f.work.join("ALSO_INJECTED").exists());
    }
}

#[test]
fn sessions_load_user_configuration_then_apply_environment_and_propagate_exit() {
    let f = Fixture::new();
    let root = f.tool(
        "custom",
        "1",
        "[default]\npaths=['bin']\n[default.env]\nSDK_MODE='active'",
    );
    executable(
        &root.join("bin/probe"),
        "#!/bin/sh\nprintf 'SDK-PROBE\\n'\n",
    );
    let config =
        "export PATH=/usr/bin:/bin\nexport USER_SETTING=loaded\nalias hello='printf ALIAS-OK'\n";
    fs::write(f.home.join(".bashrc"), config).unwrap();
    fs::write(f.home.join(".zshrc"), config).unwrap();
    for shell in SHELLS {
        let output = f.session(shell, &["shell", "custom@1"], "test \"$USER_SETTING\" = loaded || exit 41\nprobe\nhello\nloom current\nloom clear\ntest \"${SDK_MODE+x}\" != x || exit 42\nexit 7\n");
        assert_eq!(
            output.status.code(),
            Some(7),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("SDK-PROBE"), "{shell}: {stdout}");
        assert!(stdout.contains("ALIAS-OK"), "{shell}: {stdout}");
        assert!(stdout.contains("custom  1"), "{shell}: {stdout}");
    }
}

#[test]
fn zshenv_can_relocate_user_configuration_and_zdotdir_is_restored() {
    let f = Fixture::new();
    f.tool("custom", "1", "[default]\npaths=['bin']");
    let config_dir = f.home.join("config");
    fs::create_dir(&config_dir).unwrap();
    fs::write(
        f.home.join(".zshenv"),
        "export ZDOTDIR=$HOME/config\nexport ZSHENV_LOADED=yes\n",
    )
    .unwrap();
    fs::write(config_dir.join(".zshrc"), "export ZSHRC_LOADED=yes\n").unwrap();
    let output = f.session("/bin/zsh", &["shell", "custom@1"], "test \"$ZSHENV_LOADED:$ZSHRC_LOADED\" = yes:yes || exit 51\ntest \"$ZDOTDIR\" = \"$HOME/config\" || exit 52\nexit 0\n");
    success(&output);
}

#[test]
fn nested_session_gets_independent_state_and_preserves_parent() {
    let f = Fixture::new();
    f.tool(
        "one",
        "1",
        "[default]\npaths=['bin']\n[default.env]\nSDK_MODE='parent'",
    );
    f.tool(
        "two",
        "2",
        "[default]\npaths=['bin']\n[default.env]\nSDK_MODE='child'",
    );
    for shell in SHELLS {
        let script = format!(
            r#"
set -e
export SHELL={shell}
. "$INIT"
loom use one@1
PARENT_STATE=$LOOM_STATE
printf '%s\n' 'test "$SDK_MODE" = child || exit 61' 'loom clear' 'test "$SDK_MODE" = parent || exit 62' 'exit 0' | loom shell two@2
test "$SDK_MODE" = parent
test "$LOOM_STATE" = "$PARENT_STATE"
loom clear
test "${{SDK_MODE+x}}" != x
"#
        );
        success(&f.shell(shell, &script));
    }
}
