mod cli;
use anyhow::{Context, Result, bail};
use clap::Parser;
use cli::{Cli, Commands};
use loom::{config, discovery, environment, initialize, resolver, shell};
use std::{
    env,
    ffi::CString,
    fs,
    os::unix::ffi::OsStrExt,
    path::{Path, PathBuf},
    process::ExitCode,
};

fn is_executable(path: &Path) -> bool {
    let Ok(path) = CString::new(path.as_os_str().as_bytes()) else {
        return false;
    };
    // SAFETY: CString is NUL terminated and remains alive for this call.
    unsafe { libc::access(path.as_ptr(), libc::X_OK) == 0 }
}

fn which(name: &str) -> Result<PathBuf> {
    if name.is_empty() || name.contains('/') {
        bail!("Expected an executable name without a path separator");
    }
    let path = env::var_os("PATH").context("PATH is not set")?;
    for directory in env::split_paths(&path) {
        let candidate = directory.join(name);
        if let Ok(metadata) = fs::metadata(&candidate)
            && metadata.is_file()
            && is_executable(&candidate)
        {
            return Ok(if candidate.is_absolute() {
                candidate
            } else {
                env::current_dir()?.join(candidate)
            });
        }
    }
    bail!("Executable {name:?} was not found in PATH")
}

fn toolset(targets: &[String]) -> Result<resolver::Toolset> {
    resolver::resolve(
        &config::data_dir()?,
        config::requests(targets, &env::current_dir()?)?,
    )
}

fn run() -> Result<i32> {
    match Cli::parse().command {
        Commands::Init { targets, force } => initialize::initialize(
            &config::data_dir()?,
            &env::current_dir()?,
            &targets,
            force,
            &mut std::io::stdin().lock(),
            &mut std::io::stdout().lock(),
        )?,
        Commands::List { tool } => print!(
            "{}",
            discovery::list(&config::data_dir()?, tool.as_deref())?
        ),
        Commands::Which { command } => println!("{}", which(&command)?.display()),
        Commands::Use { targets } | Commands::Env { targets } => {
            print!("{}", environment::activate(&toolset(&targets)?, false)?)
        }
        Commands::Shell { targets } => return shell::session::start(&toolset(&targets)?),
        Commands::Current => print!("{}", environment::current()?),
        Commands::Clear => print!("{}", environment::clear()?),
        Commands::Version => println!("loom {}", env!("CARGO_PKG_VERSION")),
        Commands::SessionEnv { payload } => {
            let toolset = serde_json::from_slice(
                &fs::read(Path::new(&payload)).context("Cannot read session environment")?,
            )?;
            print!("{}", environment::activate(&toolset, true)?);
        }
    }
    Ok(0)
}

fn main() -> ExitCode {
    match run() {
        Ok(code) => ExitCode::from(code.clamp(0, 255) as u8),
        Err(error) => {
            eprintln!("loom: {error:#}");
            ExitCode::FAILURE
        }
    }
}
