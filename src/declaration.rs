use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::{
    collections::BTreeMap,
    fs,
    path::{Component, Path},
};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Declaration {
    pub default: Option<toml::Value>,
    #[serde(default)]
    pub versions: BTreeMap<String, toml::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Spec {
    #[serde(default)]
    pub paths: Vec<String>,
    #[serde(default)]
    pub env: BTreeMap<String, Variable>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum Variable {
    Literal(String),
    Path(PathValue),
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PathValue {
    pub path: String,
}

pub fn valid_variable(name: &str) -> bool {
    let mut chars = name.bytes();
    matches!(chars.next(), Some(c) if c.is_ascii_alphabetic() || c == b'_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == b'_')
        && name != "PATH"
        && !name.starts_with("LOOM_")
}

pub fn check_relative(value: &str) -> Result<()> {
    if value.is_empty()
        || value.contains('\0')
        || Path::new(value).components().any(|c| {
            matches!(
                c,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        bail!("Expected a relative path within the version directory, got {value:?}");
    }
    Ok(())
}

impl Declaration {
    pub fn read(path: &Path) -> Result<Self> {
        let content =
            fs::read_to_string(path).with_context(|| format!("Cannot read {}", path.display()))?;
        let declaration = toml::from_str(&content)
            .with_context(|| format!("Invalid declaration {}", path.display()))?;
        Ok(declaration)
    }

    pub fn select(&self, version: &str) -> Result<Spec> {
        let value = self
            .versions
            .get(version)
            .or(self.default.as_ref())
            .with_context(|| format!("No environment declaration for version {version}"))?;
        let spec: Spec = value
            .clone()
            .try_into()
            .with_context(|| format!("Invalid environment for version {version}"))?;
        for path in &spec.paths {
            check_relative(path)?;
        }
        for (name, value) in &spec.env {
            if !valid_variable(name) {
                bail!("Invalid or reserved environment variable {name:?}");
            }
            match value {
                Variable::Literal(value) if value.contains('\0') => {
                    bail!("Variable {name} contains a NUL character")
                }
                Variable::Path(value) => check_relative(&value.path)?,
                _ => {}
            }
        }
        Ok(spec)
    }
}
