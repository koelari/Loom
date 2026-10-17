use crate::{
    config::Requests,
    declaration::{Declaration, Variable},
    discovery::directories,
};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Toolset {
    pub tools: BTreeMap<String, String>,
    pub source: String,
    pub paths: Vec<String>,
    pub variables: BTreeMap<String, String>,
}

fn numeric(version: &str) -> Option<Vec<&str>> {
    version
        .split('.')
        .map(|part| {
            if part.is_empty() || !part.bytes().all(|c| c.is_ascii_digit()) {
                None
            } else {
                let n = part.trim_start_matches('0');
                Some(if n.is_empty() { "0" } else { n })
            }
        })
        .collect()
}

pub fn compare_versions(a: &str, b: &str) -> Ordering {
    match (numeric(a), numeric(b)) {
        (Some(a_parts), Some(b_parts)) => {
            for (a, b) in a_parts.iter().zip(b_parts.iter()) {
                let order = a.len().cmp(&b.len()).then_with(|| a.cmp(b));
                if order != Ordering::Equal {
                    return order;
                }
            }
            a_parts.len().cmp(&b_parts.len()).then_with(|| a.cmp(b))
        }
        // Keep numeric and named versions in distinct groups for a total ordering.
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        _ => a.cmp(b),
    }
}

pub fn choose_version(versions: &[String], selector: &str) -> Option<String> {
    if versions.iter().any(|v| v == selector) {
        return Some(selector.into());
    }
    let requested = numeric(selector)?;
    versions
        .iter()
        .filter(|v| numeric(v).is_some_and(|n| n.starts_with(&requested)))
        .max_by(|a, b| compare_versions(a, b))
        .cloned()
}

fn resolved_path(root: &Path, relative: &str, directory: bool) -> Result<String> {
    let path = fs::canonicalize(root.join(relative))
        .with_context(|| format!("Missing Tool path {}", root.join(relative).display()))?;
    if !path.starts_with(root) {
        bail!("Tool path escapes version directory: {}", path.display());
    }
    if directory && !path.is_dir() {
        bail!("PATH entry is not a directory: {}", path.display());
    }
    let value = path.to_str().context("Tool paths must be UTF-8")?;
    if directory && value.contains(':') {
        bail!("PATH directories cannot contain ':': {value}");
    }
    Ok(value.into())
}

pub fn resolve(data: &Path, requests: Requests) -> Result<Toolset> {
    let mut result = Toolset {
        tools: BTreeMap::new(),
        source: requests.source,
        paths: Vec::new(),
        variables: BTreeMap::new(),
    };
    let mut seen = BTreeSet::new();
    for (tool, selector) in requests.tools {
        let tool_root = data.join("tools").join(&tool);
        let versions = directories(&tool_root)?;
        let version = choose_version(&versions, &selector).with_context(|| format!(
            "No local version matches {tool}@{selector}.\nPlace the tool under: {}/<version>/\nDeclare its environment in: {}/loom.toml",
            tool_root.display(), tool_root.display()
        ))?;
        let root = fs::canonicalize(tool_root.join(&version))?;
        let declaration_path = tool_root.join("loom.toml");
        let spec = Declaration::read(&declaration_path)?
            .select(&version)
            .with_context(|| {
                format!(
                    "Cannot use {tool}@{version} from {}",
                    declaration_path.display()
                )
            })?;
        for path in spec.paths {
            let path = resolved_path(&root, &path, true)?;
            if seen.insert(path.clone()) {
                result.paths.push(path);
            }
        }
        for (name, value) in spec.env {
            let value = match value {
                Variable::Literal(value) => value,
                Variable::Path(value) => resolved_path(&root, &value.path, false)?,
            };
            if result
                .variables
                .get(&name)
                .is_some_and(|previous| previous != &value)
            {
                bail!(
                    "Conflicting values for environment variable {name} (while adding {tool}@{version})"
                );
            }
            result.variables.insert(name, value);
        }
        result.tools.insert(tool, version);
    }
    Ok(result)
}
