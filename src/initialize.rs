use crate::{
    config::{self, Requests},
    declaration::Declaration,
    discovery::directories,
    resolver, terminal_menu,
};
use anyhow::{Context, Result, bail};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{self, BufRead, IsTerminal, Write},
    path::Path,
};

use crate::terminal_menu::ToolChoice as Choice;

fn prompt(
    input: &mut impl BufRead,
    output: &mut impl Write,
    message: &str,
) -> Result<Option<String>> {
    write!(output, "{message}")?;
    output.flush()?;
    let mut line = String::new();
    if input.read_line(&mut line)? == 0 {
        return Ok(None);
    }
    let line = line.trim();
    if line.eq_ignore_ascii_case("q") {
        return Ok(None);
    }
    Ok(Some(line.to_owned()))
}

fn choose_tools(
    data: &Path,
    input: &mut impl BufRead,
    output: &mut impl Write,
    terminal: Option<&console::Term>,
) -> Result<Option<BTreeMap<String, String>>> {
    let root = data.join("tools");
    let mut choices = Vec::new();
    for tool in directories(&root)? {
        let tool_root = root.join(&tool);
        let declaration = match Declaration::read(&tool_root.join("loom.toml")) {
            Ok(declaration) => declaration,
            Err(_) => continue,
        };
        let mut versions: Vec<_> = directories(&tool_root)?
            .into_iter()
            .filter(|version| declaration.select(version).is_ok())
            .collect();
        versions.reverse();
        if !versions.is_empty() {
            choices.push(Choice { tool, versions });
        }
    }
    choices.sort_by(|a, b| a.tool.cmp(&b.tool));
    if choices.is_empty() {
        bail!(
            "No local tools with valid declarations found. Prepare tools and loom.toml under {} first",
            root.display()
        );
    }
    if let Some(terminal) = terminal {
        output.flush()?;
        return terminal_menu::tool_tree(terminal, &choices);
    }
    writeln!(output, "\nLocal tools (q to cancel):")?;
    for (index, choice) in choices.iter().enumerate() {
        writeln!(
            output,
            "  {}. {} ({})",
            index + 1,
            choice.tool,
            choice.versions.join(", ")
        )?;
    }
    let selected = loop {
        let Some(line) = prompt(
            input,
            output,
            "Select tools by number, separated by spaces or commas: ",
        )?
        else {
            return Ok(None);
        };
        let selected: Option<BTreeSet<usize>> = line
            .split(|c: char| c.is_whitespace() || c == ',')
            .filter(|part| !part.is_empty())
            .map(|part| {
                part.parse::<usize>()
                    .ok()
                    .filter(|index| *index > 0 && *index <= choices.len())
            })
            .collect();
        if let Some(selected) = selected.filter(|selected| !selected.is_empty()) {
            break selected;
        }
        writeln!(output, "Enter one or more of the displayed tool numbers.")?;
    };
    let mut tools = BTreeMap::new();
    for index in selected {
        let choice = &choices[index - 1];
        writeln!(output, "\n{} versions:", choice.tool)?;
        for (index, version) in choice.versions.iter().enumerate() {
            writeln!(output, "  {}. {version}", index + 1)?;
        }
        loop {
            let Some(line) = prompt(input, output, "Select version [1]: ")? else {
                return Ok(None);
            };
            let selected = if line.is_empty() {
                Some(1)
            } else {
                line.parse::<usize>().ok()
            };
            if let Some(index) =
                selected.filter(|index| *index > 0 && *index <= choice.versions.len())
            {
                tools.insert(choice.tool.clone(), choice.versions[index - 1].clone());
                break;
            }
            writeln!(output, "Enter one of the displayed version numbers.")?;
        }
    }
    Ok(Some(tools))
}

pub fn initialize(
    data: &Path,
    cwd: &Path,
    targets: &[String],
    force: bool,
    input: &mut impl BufRead,
    output: &mut impl Write,
) -> Result<()> {
    let path = cwd.join(".loom");
    let existing = match fs::symlink_metadata(&path) {
        Ok(metadata) => {
            if !metadata.file_type().is_file() {
                bail!(
                    "{} must be a regular file, not a directory or symlink",
                    path.display()
                );
            }
            Some(
                fs::read_to_string(&path)
                    .with_context(|| format!("Cannot read {}", path.display()))?,
            )
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(error.into()),
    };
    let interactive = targets.is_empty();
    let terminal = (interactive && io::stdin().is_terminal() && io::stdout().is_terminal())
        .then(console::Term::stdout);
    if !interactive && existing.is_some() && !force {
        bail!(
            "{} already exists; use --force to replace it",
            path.display()
        );
    }
    if interactive && terminal.is_none() {
        writeln!(output, "Initialize {}", path.display())?;
        if let Some(contents) = &existing {
            writeln!(output, "\nExisting configuration:\n{contents}")?;
        }
    }
    let tools = if interactive {
        let Some(tools) = choose_tools(data, input, output, terminal.as_ref())? else {
            writeln!(output, "Initialization cancelled.")?;
            return Ok(());
        };
        tools
    } else {
        if targets.iter().any(|target| !target.contains('@')) {
            bail!("loom init expects tool@version arguments, not a project file");
        }
        config::requests(targets, cwd)?.tools
    };
    // Validate the complete selection, including paths and conflicting variables,
    // before writing any project configuration.
    resolver::resolve(
        data,
        Requests {
            tools: tools.clone(),
            source: "explicit".into(),
        },
    )?;
    let contents = toml::to_string(&tools)?;
    if interactive {
        if let Some(terminal) = &terminal {
            output.flush()?;
            if !terminal_menu::review(terminal, &contents, existing.as_deref())? {
                writeln!(output, "Initialization cancelled.")?;
                return Ok(());
            }
        } else {
            writeln!(output, "\nProposed .loom:\n{contents}")?;
            let message = if existing.is_some() {
                "Replace existing .loom? [y/N]: "
            } else {
                "Write .loom? [Y/n]: "
            };
            loop {
                let Some(answer) = prompt(input, output, message)? else {
                    writeln!(output, "Initialization cancelled.")?;
                    return Ok(());
                };
                match answer.to_ascii_lowercase().as_str() {
                    "y" | "yes" => break,
                    "" if existing.is_none() => break,
                    "" | "n" | "no" => {
                        writeln!(output, "Initialization cancelled.")?;
                        return Ok(());
                    }
                    _ => writeln!(output, "Enter yes or no (q to cancel).")?,
                }
            }
        }
    }
    // Write in the project directory and publish only the complete file.
    let mut temporary = tempfile::NamedTempFile::new_in(cwd)?;
    temporary.write_all(contents.as_bytes())?;
    temporary.as_file().sync_all()?;
    if let Some(previous) = existing {
        let metadata = fs::symlink_metadata(&path)?;
        if !metadata.file_type().is_file() || fs::read_to_string(&path)? != previous {
            bail!(
                "{} changed during initialization; no changes were written",
                path.display()
            );
        }
        temporary
            .as_file()
            .set_permissions(metadata.permissions())?;
        temporary
            .persist(&path)
            .with_context(|| format!("Cannot replace {}", path.display()))?;
    } else {
        temporary.persist_noclobber(&path).with_context(|| {
            format!(
                "Cannot create {} (it may have been created by another process)",
                path.display()
            )
        })?;
    }
    writeln!(
        output,
        "Wrote {}\nRun `loom shell` to enter the project environment.",
        path.display()
    )?;
    Ok(())
}
