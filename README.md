# Loom

Declare local tools, select versions, and use them in your shell.

Loom reads tool directories and environment declarations. You prepare the tools;
Loom selects local versions and applies their PATH entries and environment variables.

## Build and install

Requires a Rust toolchain (Rust 1.88 or newer) and a Unix system. The current shell
integration supports Bash and Zsh.

```sh
cargo build --release --locked
./target/release/loom --help
```

To install the binary and shell integration in your user directories:

```sh
./scripts/install.sh
```

The script installs `loom` in `~/.local/bin` and the integration script in
`$HOME/.loom/shell/init.sh`. Ensure `~/.local/bin` is on
PATH. Set `LOOM_BIN_DIR` to choose another binary directory. User shell configuration
files are not modified.

Installation initializes this structure and preserves existing tools and declarations:

```text
~/.loom/
├── tools/
└── shell/
    └── init.sh
```

## Uninstall

If a Loom environment is active, run `loom clear` or exit the Loom shell first.
Then run:

```sh
./scripts/uninstall.sh
```

The script removes the installed binary and `~/.loom/shell/init.sh`. It preserves
`~/.loom/tools/`, tool declarations, and other user files. Set `LOOM_BIN_DIR` to the
same directory used during installation if you customized it. Repeated uninstall
is safe when the installed files are already absent.

Remove the `source "$HOME/.loom/shell/init.sh"` line from your shell startup
configuration and restart the shell. The uninstall script does not edit shell
configuration files.

## Prepare a tool

Place a tool's contents under its version directory, and write one `loom.toml` for
the tool:

```text
~/.loom/
└── tools/
    └── java/
        ├── loom.toml
        └── 21.0.12/
            └── bin/
                ├── java
                └── javac
```

```toml
# tools/java/loom.toml
[default]
paths = ["bin"]

[default.env]
JAVA_HOME = { path = "." }
```

Tool names come from directory names; there is no built-in tool catalog. Tools can
also be linked into the version directories. Paths inside a declaration must stay
within the selected tool's resolved version root.

Loom uses `$HOME/.loom` as its data directory. Tool
and version names consist of ASCII letters, digits, `.`, `_`, `+`, and `-`, and
cannot be `.` or `..`. Directory names starting with `.` are ignored.

## Enter a tool environment

```sh
loom list
loom shell java@21
java -version
loom current
loom which java
exit
```

`loom shell` starts an interactive shell selected by `$SHELL`. It loads your user
configuration, applies the selected tool environment, and makes `loom use` and
`loom clear` available inside the session. `exit` returns to the parent shell and
propagates the session's exit status.

The session inherits exported variables and the current directory. Aliases,
functions, and the prompt are loaded from your shell configuration. Temporary
unexported state in the parent shell is not copied. Changing directories does not
change the selected tools. Shell configuration must allow normal interactive
startup to complete; configurations that replace the shell with `exec` or disable
startup-file processing cannot participate in this initialization flow.

## Switch the current shell

Add this line to your shell startup configuration (for example, `.bashrc` or `.zshrc`):

```sh
source "$HOME/.loom/shell/init.sh"
```

After loading that configuration:

```sh
loom use java@21
loom use java@21 node@22
loom clear
```

Each `use` replaces the entire Loom tool collection. tool PATH entries precede the
original PATH. Repeated activation does not grow PATH. `clear` restores the captured
PATH and original exported variables, and unsets variables that were initially
absent. Manual PATH changes during activation are discarded by the next `use` or
`clear`.

`loom env java@21` prints environment code for explicit evaluation. Like `use`, it
outputs no partial environment on resolution failure. The generated code targets
the supported shell integration syntax.

## Project declaration

Run the interactive initializer in your project:

```sh
loom init
```

The initializer presents a directory-style tree of tools and their versions:

```text
tools/
├─ ▾ node/       ✓ 22.23.3
│    ├─ ● 22.23.3
│    └─ ○ 22.1.0
└─ ▸ java/       2 versions
```

Use Up/Down to move, Right to expand a tool or enter its versions, and Left to
return to the parent or collapse it. Space selects a version; selecting another
version of the same tool replaces the previous selection. Space on a tool toggles
its first version on, or clears its selection. Selections survive collapsing.

Enter opens a configuration preview, where arrow keys choose Save or Cancel.
Page Up/Down scrolls a long preview. Esc, `q`, or Ctrl+C cancels. The tree scrolls
within the terminal, and the footer summarizes selected tools.

Interactive choices use exact versions. When input or output is redirected, Loom
uses numbered, line-based prompts instead; the argument mode below is available
for scripts.

An existing `.loom` is displayed and requires explicit confirmation before being
replaced. EOF or cancellation leaves the project unchanged. Selected paths and
variable conflicts are validated before writing.

For scripts, specify tools directly. Selectors are kept as supplied:

```sh
loom init node@22 java@21
loom init --force node@22
```

Argument mode refuses to overwrite `.loom` unless `--force` is supplied.

You can also write the declaration directly:

```toml
# .loom
java = "21"
node = "22"
```

```sh
loom shell
# Or, with the current-shell integration:
loom use
```

Without arguments, `use`, `env`, and `shell` read `.loom` in the current directory
only. Other files can be passed explicitly: `loom shell ../.loom`.

## Environment declaration

Default settings and exact-version overrides live in the same tool file:

```toml
[default]
paths = ["bin", "tools/bin"]

[default.env]
TOOL_HOME = { path = "." }
TOOL_MODE = "development"

[versions."2.0.0"]
paths = ["runtime/bin"]

[versions."2.0.0".env]
TOOL_HOME = { path = "runtime" }
```

- `paths` lists directories relative to the selected version root.
- `env` string values are literal text; `{ path = "..." }` resolves a relative path.
- An exact-version entry replaces the entire default, including its variables.
- Absolute paths and `..` are rejected. Resolved paths must exist and stay within
  the tool version root, including through symlinks.
- `PATH` and `LOOM_*` cannot be set through `env`.
- Tools merge in name order. Repeated paths are deduplicated; conflicting variable
  values are rejected before changing the shell.

Declarations contain data, with no command substitution or shell expansion.

## Version selection and inspection

An exact directory name wins. Otherwise, numeric selectors match dot-separated
prefixes and choose the highest local numeric version. `21` can select `21.0.12`,
but cannot select `210.0.0`. Non-numeric labels such as `nightly` require an exact
match.

`loom list [tool]` reads only tool and version directories and declaration syntax;
it does not walk tool contents or validate every referenced path. Unusable
declarations are marked in the output. Activation validates the selected paths.

`loom which <command>` searches the inherited PATH in order for an executable
file. It also finds system tools; it does not inspect shell aliases or functions.

## Development

```sh
cargo test --locked
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
```

The integration tests use temporary tools and user configurations, and require
Bash and Zsh at `/bin/bash` and `/bin/zsh`. They do not modify your shell config
or real tool directory.
