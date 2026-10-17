use anyhow::Result;
use console::{Key, Term, measure_text_width, style, truncate_str};
use std::collections::BTreeMap;

pub(crate) struct ToolChoice {
    pub tool: String,
    pub versions: Vec<String>,
}

struct Screen<'a> {
    term: &'a Term,
    lines: usize,
    offset: usize,
}

impl Drop for Screen<'_> {
    fn drop(&mut self) {
        let _ = self.term.clear_last_lines(self.lines);
        let _ = self.term.show_cursor();
    }
}

fn fitted(text: &str, width: usize) -> String {
    let text = truncate_str(text, width, "…");
    format!(
        "{text}{}",
        " ".repeat(width.saturating_sub(measure_text_width(&text)))
    )
}

struct Panel<'a> {
    step: usize,
    title: &'a str,
    root: &'a str,
    rows: &'a [String],
    current: usize,
    highlight: bool,
    summary: &'a str,
    hint: &'a str,
}

impl<'a> Screen<'a> {
    fn new(term: &'a Term) -> Result<Self> {
        term.hide_cursor()?;
        Ok(Self {
            term,
            lines: 0,
            offset: 0,
        })
    }

    fn draw(&mut self, panel: Panel<'_>) -> Result<usize> {
        let Panel {
            step,
            title,
            root,
            rows,
            current,
            highlight,
            summary,
            hint,
        } = panel;
        if self.lines > 0 {
            self.term.clear_last_lines(self.lines)?;
            self.lines = 0;
        }
        let (height, width) = self.term.size();
        let width = usize::from(width.saturating_sub(4)).clamp(8, 84);
        let inner = width - 4;
        let visible = usize::from(height.saturating_sub(10))
            .max(1)
            .min(rows.len());
        if current < self.offset {
            self.offset = current;
        }
        if current >= self.offset + visible {
            self.offset = current + 1 - visible;
        }
        self.offset = self.offset.min(rows.len().saturating_sub(visible));
        let brand = format!(
            "{}  {}",
            style("LOOM").cyan().bold(),
            style("PROJECT SETUP").dim()
        );
        let heading = format!(
            "  {} {}",
            fitted(&brand, width.saturating_sub(6)),
            style(format!("{step} / 2")).cyan()
        );
        let top = format!("  {}", style(format!("╭{}╮", "─".repeat(width - 2))).dim());
        let bottom = format!("  {}", style(format!("╰{}╯", "─".repeat(width - 2))).dim());
        let mut lines = vec![
            heading,
            format!("  {}", style(title).bold()),
            String::new(),
            top,
        ];
        let root = if visible < rows.len() {
            format!(
                "{root}  ·  {}–{} / {}",
                self.offset + 1,
                self.offset + visible,
                rows.len()
            )
        } else {
            root.to_owned()
        };
        lines.push(format!(
            "  {} {} {}",
            style("│").dim(),
            fitted(&style(root).dim().to_string(), inner),
            style("│").dim()
        ));
        for (index, row) in rows.iter().enumerate().skip(self.offset).take(visible) {
            let marker = if highlight && index == current {
                style("›").cyan().bold().to_string()
            } else {
                " ".into()
            };
            let row = format!("{marker} {row}");
            lines.push(format!(
                "  {} {} {}",
                style("│").dim(),
                fitted(&row, inner),
                style("│").dim()
            ));
        }
        lines.push(bottom);
        lines.push(format!("  {}", fitted(summary, width)));
        lines.push(format!("  {}", style(truncate_str(hint, width, "…")).dim()));
        lines.push(format!(
            "  {}",
            style(if step == 1 {
                "Enter  review    Esc / q  cancel"
            } else {
                "Enter  confirm    Esc / q  cancel"
            })
            .dim()
        ));
        for line in lines {
            self.term.write_line(&truncate_str(
                &line,
                usize::from(self.term.size().1.saturating_sub(1)).max(1),
                "…",
            ))?;
            self.lines += 1;
        }
        Ok(visible)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct Row {
    tool: usize,
    version: Option<usize>,
}

fn tree_rows(choices: &[ToolChoice], expanded: &[bool]) -> Vec<Row> {
    let mut rows = Vec::new();
    for (tool, choice) in choices.iter().enumerate() {
        rows.push(Row {
            tool,
            version: None,
        });
        if expanded[tool] {
            rows.extend((0..choice.versions.len()).map(|version| Row {
                tool,
                version: Some(version),
            }));
        }
    }
    rows
}

pub(crate) fn tool_tree(
    term: &Term,
    choices: &[ToolChoice],
) -> Result<Option<BTreeMap<String, String>>> {
    let mut screen = Screen::new(term)?;
    let mut expanded = vec![false; choices.len()];
    expanded[0] = true;
    let mut selected = vec![None; choices.len()];
    let mut current = 0;
    let mut message = None;
    loop {
        let rows = tree_rows(choices, &expanded);
        let labels: Vec<_> = rows
            .iter()
            .map(|row| {
                let choice = &choices[row.tool];
                let last_tool = row.tool + 1 == choices.len();
                match row.version {
                    None => {
                        let branch = style(if last_tool { "└─" } else { "├─" }).dim();
                        let arrow = if expanded[row.tool] { "▾" } else { "▸" };
                        let badge = match selected[row.tool] {
                            Some(index) => style(format!("✓ {}", choice.versions[index]))
                                .green()
                                .to_string(),
                            None => style(format!(
                                "{} version{}",
                                choice.versions.len(),
                                if choice.versions.len() == 1 { "" } else { "s" }
                            ))
                            .dim()
                            .to_string(),
                        };
                        format!(
                            "{branch} {arrow} {}  {badge}",
                            style(format!("{}/", choice.tool)).bold()
                        )
                    }
                    Some(index) => {
                        let stem = if last_tool { "  " } else { "│ " };
                        let branch = if index + 1 == choice.versions.len() {
                            "└─"
                        } else {
                            "├─"
                        };
                        let checked = selected[row.tool] == Some(index);
                        let marker = if checked {
                            style("●").green().bold()
                        } else {
                            style("○").dim()
                        };
                        let version = if checked {
                            style(&choice.versions[index]).green().bold()
                        } else {
                            style(&choice.versions[index])
                        };
                        format!(
                            "{} {marker} {version}",
                            style(format!("{stem}   {branch}")).dim()
                        )
                    }
                }
            })
            .collect();
        let selection: Vec<_> = choices
            .iter()
            .enumerate()
            .filter_map(|(index, choice)| {
                selected[index]
                    .map(|version| format!("{}@{}", choice.tool, choice.versions[version]))
            })
            .collect();
        let summary = message.take().unwrap_or_else(|| {
            if selection.is_empty() {
                style("No tools selected · choose one version per tool")
                    .dim()
                    .to_string()
            } else {
                format!(
                    "{}  {}",
                    style(format!("{} selected", selection.len())).green(),
                    selection.join(" · ")
                )
            }
        });
        let visible = screen.draw(Panel {
            step: 1,
            title: "Select tools & versions",
            root: "tools/",
            rows: &labels,
            current,
            highlight: true,
            summary: &summary,
            hint: "↑↓ move   ←→ collapse / expand   Space select",
        })?;
        let row = rows[current];
        match term.read_key_raw()? {
            Key::ArrowUp | Key::BackTab => current = (current + rows.len() - 1) % rows.len(),
            Key::ArrowDown | Key::Tab => current = (current + 1) % rows.len(),
            Key::Home => current = 0,
            Key::End => current = rows.len() - 1,
            Key::PageUp => current = current.saturating_sub(visible),
            Key::PageDown => current = (current + visible).min(rows.len() - 1),
            Key::ArrowRight => {
                if row.version.is_none() {
                    if expanded[row.tool] {
                        current += 1;
                    } else {
                        expanded[row.tool] = true;
                    }
                }
            }
            Key::ArrowLeft => {
                if row.version.is_some() {
                    current = rows
                        .iter()
                        .position(|r| r.tool == row.tool && r.version.is_none())
                        .unwrap();
                } else {
                    expanded[row.tool] = false;
                }
            }
            Key::Char(' ') => {
                let version = row.version.unwrap_or(0);
                selected[row.tool] = if row.version.is_none() && selected[row.tool].is_some()
                    || selected[row.tool] == Some(version)
                {
                    None
                } else {
                    Some(version)
                };
                expanded[row.tool] = true;
            }
            Key::Enter if selection.is_empty() => {
                message = Some(
                    style("Select a version with Space before continuing.")
                        .yellow()
                        .to_string(),
                );
            }
            Key::Enter => {
                return Ok(Some(
                    choices
                        .iter()
                        .enumerate()
                        .filter_map(|(index, choice)| {
                            selected[index].map(|version| {
                                (choice.tool.clone(), choice.versions[version].clone())
                            })
                        })
                        .collect(),
                ));
            }
            Key::Escape | Key::CtrlC | Key::Char('q' | 'Q' | '\x04') => return Ok(None),
            _ => {}
        }
    }
}

pub(crate) fn review(term: &Term, contents: &str, existing: Option<&str>) -> Result<bool> {
    let mut screen = Screen::new(term)?;
    let mut rows = Vec::new();
    if let Some(previous) = existing {
        rows.push(style("Current configuration").dim().to_string());
        rows.extend(
            previous
                .lines()
                .map(|line| style(format!("  {line}")).dim().to_string()),
        );
        rows.push(String::new());
    }
    rows.push(style("Proposed configuration").cyan().to_string());
    rows.extend(contents.lines().map(|line| match line.split_once(" = ") {
        Some((key, value)) => format!("  {} = {}", style(key).bold(), style(value).green()),
        None => format!("  {line}"),
    }));
    let mut action = usize::from(existing.is_some());
    let mut preview_row = 0;
    loop {
        let save = if existing.is_some() {
            "Replace .loom"
        } else {
            "Save .loom"
        };
        let actions = if action == 0 {
            format!(
                "{}    {}",
                style(format!("› {save}")).cyan().bold(),
                style("  Cancel").dim()
            )
        } else {
            format!(
                "{}    {}",
                style(format!("  {save}")).dim(),
                style("› Cancel").cyan().bold()
            )
        };
        let visible = screen.draw(Panel {
            step: 2,
            title: "Review project configuration",
            root: ".loom",
            rows: &rows,
            current: preview_row,
            highlight: false,
            summary: &actions,
            hint: "Arrow keys choose an action   PgUp/PgDn scroll preview",
        })?;
        match term.read_key_raw()? {
            Key::ArrowUp
            | Key::ArrowDown
            | Key::ArrowLeft
            | Key::ArrowRight
            | Key::Tab
            | Key::BackTab => action = 1 - action,
            Key::PageUp => preview_row = preview_row.saturating_sub(visible),
            Key::PageDown => preview_row = (preview_row + visible).min(rows.len() - 1),
            Key::Home => action = 0,
            Key::End => action = 1,
            Key::Enter => return Ok(action == 0),
            Key::Escape | Key::CtrlC | Key::Char('q' | 'Q' | '\x04') => return Ok(false),
            _ => {}
        }
    }
}
