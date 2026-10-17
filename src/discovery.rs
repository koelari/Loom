use crate::{
    config::{check_component, valid_component},
    declaration::Declaration,
    resolver::compare_versions,
};
use anyhow::{Context, Result};
use std::{fs, io::ErrorKind, path::Path};

pub fn directories(root: &Path) -> Result<Vec<String>> {
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error).with_context(|| format!("Cannot read {}", root.display())),
    };
    let mut names = Vec::new();
    for entry in entries {
        let entry = entry?;
        if entry.path().is_dir() {
            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| anyhow::anyhow!("Non-UTF-8 directory in {}", root.display()))?;
            if valid_component(&name) && !name.starts_with('.') {
                names.push(name);
            }
        }
    }
    names.sort_by(|a, b| compare_versions(a, b));
    Ok(names)
}

pub fn list(data: &Path, tool: Option<&str>) -> Result<String> {
    let tools_root = data.join("tools");
    let tools = if let Some(tool) = tool {
        check_component(tool)?;
        vec![tool.into()]
    } else {
        directories(&tools_root)?
    };
    let mut output = String::new();
    for tool in tools {
        let root = tools_root.join(&tool);
        let versions = directories(&root)?;
        if versions.is_empty() {
            continue;
        }
        let declaration = Declaration::read(&root.join("loom.toml"));
        output.push_str(&format!("{tool}\n"));
        for version in versions {
            let result = match &declaration {
                Ok(declaration) => declaration.select(&version).map(|_| ()),
                Err(error) => Err(anyhow::anyhow!("{error:#}")),
            };
            match result {
                Ok(()) => output.push_str(&format!("  {version}\n")),
                Err(error) => output.push_str(&format!("  {version}  [unavailable: {error:#}]\n")),
            }
        }
    }
    if output.is_empty() {
        output.push_str("No local tool versions found.\n");
    }
    Ok(output)
}
