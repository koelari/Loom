use anyhow::{Context, Result, bail};
use std::{
    collections::BTreeMap,
    env, fs,
    path::{Path, PathBuf},
};

pub fn data_dir() -> Result<PathBuf> {
    let home = PathBuf::from(env::var_os("HOME").context("HOME is not set")?);
    if !home.is_absolute() {
        bail!("HOME must be an absolute path: {}", home.display());
    }
    Ok(home.join(".loom"))
}

pub fn valid_component(value: &str) -> bool {
    !value.is_empty()
        && value != "."
        && value != ".."
        && value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"._+-".contains(&c))
}

pub fn check_component(value: &str) -> Result<()> {
    if !valid_component(value) {
        bail!("Invalid tool or version name: {value:?}");
    }
    Ok(())
}

#[derive(Debug)]
pub struct Requests {
    pub tools: BTreeMap<String, String>,
    pub source: String,
}

pub fn requests(args: &[String], cwd: &Path) -> Result<Requests> {
    if args.is_empty() || (args.len() == 1 && !args[0].contains('@')) {
        let file = cwd.join(args.first().map(String::as_str).unwrap_or(".loom"));
        let content = fs::read_to_string(&file).with_context(|| {
            format!(
                "Cannot read project file {} (only the current directory is searched)",
                file.display()
            )
        })?;
        let tools: BTreeMap<String, String> = toml::from_str(&content).with_context(|| {
            format!(
                "Invalid project file {}: expected tool = \"version\"",
                file.display()
            )
        })?;
        validate_requests(&tools)?;
        return Ok(Requests {
            tools,
            source: fs::canonicalize(file)?.to_string_lossy().into_owned(),
        });
    }
    let mut tools = BTreeMap::new();
    for arg in args {
        let (tool, version) = arg
            .split_once('@')
            .context("Expected tool@version, or a single project file")?;
        check_component(tool)?;
        check_component(version)?;
        if tools.insert(tool.to_owned(), version.to_owned()).is_some() {
            bail!("Tool {tool:?} was requested more than once");
        }
    }
    validate_requests(&tools)?;
    Ok(Requests {
        tools,
        source: "explicit".into(),
    })
}

fn validate_requests(tools: &BTreeMap<String, String>) -> Result<()> {
    if tools.is_empty() {
        bail!("The tool collection is empty");
    }
    for (tool, version) in tools {
        check_component(tool)?;
        check_component(version)?;
    }
    Ok(())
}
