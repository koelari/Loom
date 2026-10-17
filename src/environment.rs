use crate::{
    declaration::valid_variable,
    resolver::Toolset,
    shell::renderer::{assign, unset},
};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, env};

pub const STATE_VARIABLE: &str = "LOOM_STATE";

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct State {
    schema: u32,
    base_path: Option<String>,
    originals: BTreeMap<String, Option<String>>,
    pub tools: BTreeMap<String, String>,
    pub source: String,
}

fn value(name: &str) -> Result<Option<String>> {
    match env::var(name) {
        Ok(value) => Ok(Some(value)),
        Err(env::VarError::NotPresent) => Ok(None),
        Err(error) => Err(error).with_context(|| format!("{name} must contain UTF-8 text")),
    }
}

pub fn read_state() -> Result<Option<State>> {
    let Some(raw) = value(STATE_VARIABLE)? else {
        return Ok(None);
    };
    let state: State = serde_json::from_str(&raw)
        .context("Invalid LOOM_STATE; unset LOOM_STATE to discard corrupt session state")?;
    if state.schema != 1 || state.originals.keys().any(|key| !valid_variable(key)) {
        bail!("Invalid or unsupported LOOM_STATE");
    }
    // State is data, including when inherited from another process.
    if state
        .base_path
        .iter()
        .chain(state.originals.values().flatten())
        .any(|v| v.contains('\0'))
    {
        bail!("Invalid NUL character in LOOM_STATE");
    }
    Ok(Some(state))
}

pub fn activate(toolset: &Toolset, fresh: bool) -> Result<String> {
    let mut state = if fresh { None } else { read_state()? }.unwrap_or(State {
        schema: 1,
        base_path: value("PATH")?,
        originals: BTreeMap::new(),
        tools: BTreeMap::new(),
        source: String::new(),
    });
    let mut code = String::new();
    for name in toolset.variables.keys() {
        if !valid_variable(name) {
            bail!("Invalid or reserved variable {name:?}");
        }
        if !state.originals.contains_key(name) {
            state.originals.insert(name.clone(), value(name)?);
        }
    }
    for (name, original) in &state.originals {
        code.push_str(&assign(name, original.as_deref())?);
    }
    for (name, value) in &toolset.variables {
        code.push_str(&assign(name, Some(value))?);
    }
    let mut entries = toolset.paths.clone();
    if let Some(base) = &state.base_path {
        entries.push(base.clone());
    }
    let path = if entries.is_empty() {
        None
    } else {
        Some(entries.join(":"))
    };
    code.push_str(&assign("PATH", path.as_deref())?);
    state.tools = toolset.tools.clone();
    state.source = toolset.source.clone();
    code.push_str(&assign(
        STATE_VARIABLE,
        Some(&serde_json::to_string(&state)?),
    )?);
    Ok(code)
}

pub fn clear() -> Result<String> {
    let Some(state) = read_state()? else {
        return Ok(String::new());
    };
    let mut code = String::new();
    code.push_str(&assign("PATH", state.base_path.as_deref())?);
    for (name, original) in &state.originals {
        code.push_str(&assign(name, original.as_deref())?);
    }
    code.push_str(&unset(STATE_VARIABLE));
    Ok(code)
}

pub fn current() -> Result<String> {
    let Some(state) = read_state()? else {
        return Ok("No active Loom environment.\n".into());
    };
    let mut output = String::from("Loom environment\n\n");
    for (tool, version) in state.tools {
        output.push_str(&format!("{tool}  {version}\n"));
    }
    output.push_str(&format!("\nsource:\n  {}\n", state.source));
    Ok(output)
}
