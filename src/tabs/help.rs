use opaline::Theme;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use super::{pane_style, tab_content_block, tab_title_style};

pub(crate) const TITLE: &str = "Help";

/// Internal helper for max scroll.
pub(crate) fn max_scroll(area: Rect, theme: &Theme) -> usize {
    help_lines(theme)
        .len()
        .saturating_sub(usize::from(tab_content_block(theme).inner(area).height))
}

/// Internal helper for render.
pub(crate) fn render(frame: &mut Frame, area: Rect, theme: &Theme, scroll: usize) {
    let max_scroll = max_scroll(area, theme);
    let scroll = scroll.min(max_scroll).min(usize::from(u16::MAX)) as u16;
    frame.render_widget(
        Paragraph::new(help_lines(theme))
            .scroll((scroll, 0))
            .style(pane_style(theme))
            .block(
                tab_content_block(theme)
                    .title_style(tab_title_style(theme))
                    .title(TITLE),
            ),
        area,
    );
}

fn help_lines(theme: &Theme) -> Vec<Line<'static>> {
    let key = Style::default()
        .fg(theme.color("accent.primary").into())
        .add_modifier(Modifier::BOLD);
    let mut lines = vec![
        Line::styled("Startup options", theme.style("keyword")),
        shortcut("--uri <URI>", "Lance dataset URI", key),
        shortcut("-t, --theme", "light or dark (default: dark)", key),
        shortcut("-h, --help", "print command-line help", key),
        Line::from(""),
        Line::styled("Keyboard", theme.style("keyword")),
        shortcut("← / →, h / l", "switch focus / change tab", key),
        shortcut("Tab", "next tab from the tab pane", key),
        shortcut("↑ / ↓, j / k", "navigate", key),
        shortcut("Enter", "open / expand / collapse", key),
        shortcut("Esc", "parent widget / tab pane", key),
        shortcut("t", "toggle theme", key),
        shortcut("q", "quit", key),
        Line::from(""),
        Line::styled("Tabs", theme.style("keyword")),
    ];
    lines.extend(tab_description(
        "Dataset Overview",
        &["Shows dataset identity, storage summary, and the top-level schema."],
        key,
    ));
    lines.extend(tab_description(
        "Tables",
        &["Select a version, then inspect its manifest, schema fields, and fragments."],
        key,
    ));
    lines.extend(tab_description(
        "Data Files",
        &[
            "Browse data files by version and fragment.",
            "Inspect the selected file's layout outline and byte layout.",
        ],
        key,
    ));
    lines.extend(tab_description(
        "Indices",
        &[
            "Browse indices for a version.",
            "Inspect index metadata and individual index file layouts.",
        ],
        key,
    ));
    lines.extend(tab_description(
        "Storage Layout",
        &[
            "Browse the dataset's stored paths and directory structure.",
            "Enter expands or collapses the selected directory.",
        ],
        key,
    ));
    lines.extend(tab_description(
        "Help",
        &["Lists startup options, keyboard shortcuts, and tab descriptions."],
        key,
    ));
    lines
}

fn shortcut(key: &str, description: &str, key_style: Style) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{key:<14}"), key_style),
        Span::from(description.to_string()),
    ])
}

fn tab_description(title: &str, details: &[&str], title_style: Style) -> Vec<Line<'static>> {
    let mut lines = Vec::with_capacity(details.len() + 2);
    lines.push(Line::styled(title.to_string(), title_style));
    lines.extend(
        details
            .iter()
            .map(|detail| Line::from(format!("  {detail}"))),
    );
    lines.push(Line::from(""));
    lines
}
