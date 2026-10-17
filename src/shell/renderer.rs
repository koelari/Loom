use anyhow::{Result, bail};

pub fn quote(value: &str) -> Result<String> {
    if value.contains('\0') {
        bail!("Shell values cannot contain NUL characters");
    }
    Ok(format!("'{}'", value.replace('\'', "'\\''")))
}

pub fn unset(name: &str) -> String {
    format!("unset {name}\n")
}

pub fn assign(name: &str, value: Option<&str>) -> Result<String> {
    match value {
        Some(value) => Ok(format!("export {name}={}\n", quote(value)?)),
        None => Ok(unset(name)),
    }
}
