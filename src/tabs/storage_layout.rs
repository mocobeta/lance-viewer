use crate::{
    app::DatasetState,
    dataset::{StorageLayout, StorageNode},
    formatting::format_bytes,
};
use opaline::Theme;
use ratatui::{Frame, layout::Rect, text::Line, widgets::Paragraph};
use std::collections::BTreeSet;

use super::{tab_content_block, tab_title_style};

pub(crate) const TITLE: &str = "Storage Layout";
pub(crate) const CONTENT: &str = "No storage layout loaded.";

/// Internal helper for render.
pub(crate) fn render(
    frame: &mut Frame,
    area: Rect,
    theme: &Theme,
    dataset_state: &DatasetState,
    expanded: &BTreeSet<String>,
    selected: &str,
    focused: bool,
) {
    let lines = match dataset_state {
        DatasetState::Loaded(dataset_info) => tree_lines(
            &dataset_info.storage_layout,
            expanded,
            Some(selected),
            theme,
        ),
        DatasetState::Unavailable { uri, reason } => vec![
            Line::styled("Unable to load storage layout", theme.style("keyword")),
            Line::from(format!("URI: {uri}")),
            Line::from(""),
            Line::from(reason.clone()),
        ],
        DatasetState::Loading => vec![Line::from("Loading dataset...")],
        DatasetState::Empty => vec![Line::from(CONTENT)],
    };
    frame.render_widget(
        Paragraph::new(lines).style(super::pane_style(theme)).block(
            tab_content_block(theme)
                .title_style(tab_title_style(theme))
                .border_style(if focused {
                    theme.style("focused_border")
                } else {
                    theme.style("unfocused_border")
                })
                .title(TITLE),
        ),
        area,
    );
}

fn tree_lines(
    layout: &StorageLayout,
    expanded: &BTreeSet<String>,
    selected: Option<&str>,
    theme: &Theme,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    for (root_index, root) in layout.tree().iter().enumerate() {
        if root_index > 0 {
            lines.push(Line::from(""));
        }
        lines.push(tree_line(
            root.name.clone(),
            selected == Some(root.key.as_str()),
            theme,
        ));
        if expanded.contains(&root.key) {
            let children: Vec<_> = root.children.values().collect();
            render_children(&children, "", expanded, selected, theme, &mut lines);
        }
    }
    lines
}

fn render_children(
    children: &[&StorageNode],
    prefix: &str,
    expanded: &BTreeSet<String>,
    selected: Option<&str>,
    theme: &Theme,
    lines: &mut Vec<Line<'static>>,
) {
    for (index, node) in children.iter().enumerate() {
        let last = index == children.len() - 1;
        let indicator = if node.directory {
            if expanded.contains(&node.key) {
                "▾ "
            } else {
                "▸ "
            }
        } else {
            ""
        };
        lines.push(tree_line(
            format!(
                "{prefix}{}{}{}{}",
                if last { "└── " } else { "├── " },
                indicator,
                node.name,
                node.size_bytes
                    .map(|size| format!("  {}", format_bytes(size)))
                    .unwrap_or_default()
            ),
            selected == Some(node.key.as_str()),
            theme,
        ));
        if node.directory && expanded.contains(&node.key) {
            let child_prefix = format!("{prefix}{}", if last { "    " } else { "│   " });
            let children: Vec<_> = node.children.values().collect();
            render_children(&children, &child_prefix, expanded, selected, theme, lines);
        }
    }
}

fn tree_line(text: String, selected: bool, theme: &Theme) -> Line<'static> {
    let mut style = super::pane_style(theme);
    if selected {
        style = style
            .fg(theme.color("accent.primary").into())
            .bg(theme.color("bg.active").into());
    }
    Line::styled(text, style)
}

#[cfg(test)]
mod tests {
    use super::tree_lines;
    use crate::dataset::{StorageEntry, StorageLayout, StorageRoot};
    use opaline::Theme;
    use std::collections::BTreeSet;

    #[test]
    fn renders_tree_command_style() {
        let layout = StorageLayout {
            roots: vec![StorageRoot {
                name: ".".to_string(),
                entries: vec![
                    StorageEntry {
                        path: "data/1.lance".to_string(),
                        directory: false,
                        size_bytes: Some(1536),
                    },
                    StorageEntry {
                        path: "_versions/0.manifest".to_string(),
                        directory: false,
                        size_bytes: Some(512),
                    },
                ],
            }],
        };

        let lines: Vec<_> = tree_lines(
            &layout,
            &BTreeSet::from(["root:.".to_string()]),
            None,
            &Theme::default(),
        )
        .into_iter()
        .map(|line| line.to_string())
        .collect();

        assert_eq!(lines, [".", "├── ▸ _versions", "└── ▸ data"]);
    }

    #[test]
    fn renders_expanded_nodes_and_children() {
        let layout = StorageLayout {
            roots: vec![StorageRoot {
                name: ".".to_string(),
                entries: vec![
                    StorageEntry {
                        path: "data/1.lance".to_string(),
                        directory: false,
                        size_bytes: Some(1536),
                    },
                    StorageEntry {
                        path: "_versions/0.manifest".to_string(),
                        directory: false,
                        size_bytes: Some(512),
                    },
                ],
            }],
        };
        let expanded = layout.default_expanded();

        let lines: Vec<_> = tree_lines(&layout, &expanded, None, &Theme::default())
            .into_iter()
            .map(|line| line.to_string())
            .collect();

        assert_eq!(
            lines,
            [
                ".",
                "├── ▾ _versions",
                "│   └── 0.manifest  512 B",
                "└── ▾ data",
                "    └── 1.lance  1.5 KiB"
            ]
        );
    }
}
