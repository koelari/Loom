use crate::{resolver::Toolset, shell::renderer::quote};
use anyhow::{Context, Result, bail};
use std::{
    env, fs,
    os::unix::process::{CommandExt, ExitStatusExt},
    path::Path,
    process::Command,
};

const INIT: &str = include_str!("../../shell/init.sh");

fn quoted_path(path: &Path) -> Result<String> {
    quote(path.to_str().context("Shell startup paths must be UTF-8")?)
}

pub fn start(toolset: &Toolset) -> Result<i32> {
    let shell =
        env::var_os("SHELL").context("SHELL is not set; select a supported shell (bash or zsh)")?;
    let shell = Path::new(&shell);
    if !shell.is_absolute() {
        bail!("SHELL must be an absolute executable path");
    }
    let name = shell
        .file_name()
        .and_then(|v| v.to_str())
        .context("Invalid SHELL path")?;
    if !matches!(name, "bash" | "zsh") {
        bail!("Unsupported shell {name:?}; session startup currently supports bash and zsh");
    }
    let home = env::var_os("HOME").context("HOME is not set")?;
    let temp = tempfile::Builder::new().prefix("loom-session-").tempdir()?;
    let init_path = temp.path().join("init.sh");
    let payload_path = temp.path().join("toolset.json");
    let binary = env::current_exe()?;
    fs::write(&init_path, INIT)?;
    fs::write(&payload_path, serde_json::to_vec(toolset)?)?;
    // Embed the bootstrap paths in the temporary startup file rather than relying
    // on environment variables that a user's configuration may overwrite.
    let apply = format!(
        "\nexport LOOM_EXECUTABLE={}\n. {}\n__loom_startup_code=\"$({} __session-env {})\" || exit 1\neval \"$__loom_startup_code\" || exit 1\nunset __loom_startup_code\n",
        quoted_path(&binary)?,
        quoted_path(&init_path)?,
        quoted_path(&binary)?,
        quoted_path(&payload_path)?
    );
    let mut command = Command::new(shell);
    // Parent session snapshots are never imported as the child's own snapshot.
    command.env_remove("LOOM_STATE");
    match name {
        "bash" => {
            let rc = temp.path().join("bashrc");
            fs::write(
                &rc,
                format!(
                    "if [ -r {home}/.bashrc ]; then . {home}/.bashrc; fi\n{apply}",
                    home = quoted_path(Path::new(&home))?
                ),
            )?;
            command.arg("--rcfile").arg(rc).arg("-i");
        }
        "zsh" => {
            let original = match env::var("ZDOTDIR") {
                Ok(value) => format!("export ZDOTDIR={}\n", quote(&value)?),
                Err(env::VarError::NotPresent) => "unset ZDOTDIR\n".into(),
                Err(error) => return Err(error).context("ZDOTDIR must be UTF-8"),
            };
            let zshenv = format!(
                "{original}if [[ -r ${{ZDOTDIR-$HOME}}/.zshenv ]]; then source \"${{ZDOTDIR-$HOME}}/.zshenv\"; fi\n__loom_zdotdir_set=${{ZDOTDIR+x}}\n__loom_zdotdir=${{ZDOTDIR-}}\nexport ZDOTDIR={}\n",
                quoted_path(temp.path())?
            );
            let zshrc = format!(
                "if [[ $__loom_zdotdir_set = x ]]; then export ZDOTDIR=$__loom_zdotdir; else unset ZDOTDIR; fi\nunset __loom_zdotdir_set __loom_zdotdir\nif [[ -r ${{ZDOTDIR-$HOME}}/.zshrc ]]; then source \"${{ZDOTDIR-$HOME}}/.zshrc\"; fi\n{apply}"
            );
            fs::write(temp.path().join(".zshenv"), zshenv)?;
            fs::write(temp.path().join(".zshrc"), zshrc)?;
            command.env("ZDOTDIR", temp.path()).arg("-i");
        }
        _ => unreachable!(),
    }
    // The interactive shell and its foreground jobs receive terminal signals.
    // Keep the waiting supervisor alive when a foreground job is interrupted.
    let _signals = TerminalSignals::ignore()?;
    // SAFETY: the callback only calls async-signal-safe signal() before exec.
    unsafe {
        command.pre_exec(|| {
            libc::signal(libc::SIGINT, libc::SIG_DFL);
            libc::signal(libc::SIGQUIT, libc::SIG_DFL);
            Ok(())
        });
    }
    let status = command
        .status()
        .with_context(|| format!("Cannot start {}", shell.display()))?;
    Ok(status
        .code()
        .unwrap_or_else(|| 128 + status.signal().unwrap_or(1)))
}

struct TerminalSignals {
    interrupt: libc::sighandler_t,
    quit: libc::sighandler_t,
}

impl TerminalSignals {
    fn ignore() -> Result<Self> {
        // SAFETY: this single-threaded CLI installs only the OS ignore handler.
        let interrupt = unsafe { libc::signal(libc::SIGINT, libc::SIG_IGN) };
        if interrupt == libc::SIG_ERR {
            return Err(std::io::Error::last_os_error().into());
        }
        // SAFETY: same as above.
        let quit = unsafe { libc::signal(libc::SIGQUIT, libc::SIG_IGN) };
        if quit == libc::SIG_ERR {
            let error = std::io::Error::last_os_error();
            // SAFETY: restore the previously returned handler.
            unsafe {
                libc::signal(libc::SIGINT, interrupt);
            }
            return Err(error.into());
        }
        Ok(Self { interrupt, quit })
    }
}

impl Drop for TerminalSignals {
    fn drop(&mut self) {
        // SAFETY: restore the handlers obtained from signal() in ignore().
        unsafe {
            libc::signal(libc::SIGINT, self.interrupt);
            libc::signal(libc::SIGQUIT, self.quit);
        }
    }
}
