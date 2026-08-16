use crate::{app::DatasetState, dataset::DatasetInfo, formatting::format_bytes};
use opaline::Theme;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    symbols::line::HORIZONTAL,
    text::{Line, Span},
    widgets::{Cell, Paragraph, Row, Table},
};

use super::{tab_content_block, tab_title_style};

pub(crate) const TITLE: &str = "Dataset Overview";

pub(crate) const CONTENT: &str = r#"______
___  / ______ __________________
__  /  _  __ `/_  __ \  ___/  _ \
_  /___/ /_/ /_  / / / /__ /  __/
/_____/\__,_/ /_/ /_/\___/ \___/"#;

const SCHEMA_COLUMN_WIDTHS: [Constraint; 5] = [
    Constraint::Percentage(8),
    Constraint::Percentage(32),
    Constraint::Percentage(25),
    Constraint::Percentage(18),
    Constraint::Percentage(17),
];

pub(crate) fn render(frame: &mut Frame, area: Rect, theme: &Theme, dataset_state: &DatasetState) {
    match dataset_state {
        DatasetState::Loaded(dataset_info) => render_loaded(frame, area, theme, dataset_info),
        DatasetState::Unavailable { uri, reason } => {
            let mut lines = vec![
                Line::styled("Unable to open dataset", theme.style("keyword")),
                Line::from(format!("URI: {uri}")),
                Line::from(""),
                Line::styled("Reason:", theme.style("keyword")),
            ];
            lines.extend(
                reason
                    .lines()
                    .map(|line| Line::styled(format!("  {line}"), theme.style("muted"))),
            );
            lines.push(Line::from(""));
            lines.extend(CONTENT.lines().map(Line::from));
            render_text(frame, area, theme, lines);
        }
        DatasetState::Loading => {
            let mut lines = vec![Line::styled("Loading dataset...", theme.style("keyword"))];
            lines.push(Line::from(""));
            lines.extend(CONTENT.lines().map(Line::from));
            render_text(frame, area, theme, lines);
        }
        DatasetState::Empty => render_text(
            frame,
            area,
            theme,
            CONTENT.lines().map(Line::from).collect(),
        ),
    }
}

fn render_loaded(frame: &mut Frame, area: Rect, theme: &Theme, dataset_info: &DatasetInfo) {
    let block = tab_content_block(theme)
        .title_style(tab_title_style(theme))
        .title(TITLE);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let properties_lines = properties(dataset_info, theme);
    let info_height = properties_lines.len() as u16;
    let [info_area, schema_area] =
        Layout::vertical([Constraint::Length(info_height), Constraint::Min(0)]).areas(inner);

    frame.render_widget(
        Paragraph::new(properties_lines).style(super::pane_style(theme)),
        info_area,
    );

    let [schema_title_area, table_area] =
        Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).areas(schema_area);
    frame.render_widget(
        Paragraph::new(Line::styled("Schema", property_label_style(theme)))
            .style(super::pane_style(theme)),
        schema_title_area,
    );
    frame.render_widget(schema_table(dataset_info, theme), table_area);
    if table_area.height > 1 {
        frame.render_widget(
            Paragraph::new(Line::styled(
                HORIZONTAL.repeat(table_area.width as usize),
                theme.style("muted"),
            )),
            Rect::new(table_area.x, table_area.y + 1, table_area.width, 1),
        );
    }
}

fn render_text(frame: &mut Frame, area: Rect, theme: &Theme, lines: Vec<Line<'static>>) {
    frame.render_widget(
        Paragraph::new(lines).style(super::pane_style(theme)).block(
            tab_content_block(theme)
                .title_style(tab_title_style(theme))
                .title(TITLE),
        ),
        area,
    );
}

fn properties(dataset_info: &DatasetInfo, theme: &Theme) -> Vec<Line<'static>> {
    let label = property_label_style(theme);
    let value: ratatui::style::Style = theme.style("text.primary").into();
    let lines = vec![
        property("URI", &dataset_info.uri, label, value),
        property("Storage format", &dataset_info.storage_format, label, value),
        property("Writer", &dataset_info.writer_version, label, value),
        property("Branch", &dataset_info.branch, label, value),
        property("Tag", &dataset_info.tag, label, value),
        property(
            "History",
            &format!(
                "{} versions, latest {}{}",
                dataset_info.version_count,
                dataset_info.latest_version,
                if dataset_info.stale { " (stale)" } else { "" }
            ),
            label,
            value,
        ),
        property("Committed", &dataset_info.timestamp, label, value),
        property("Total rows", &dataset_info.rows.to_string(), label, value),
        property("Columns", &dataset_info.columns.to_string(), label, value),
        property(
            "Fragments",
            &dataset_info.fragments.to_string(),
            label,
            value,
        ),
        property(
            "Data files",
            &dataset_info.data_files.to_string(),
            label,
            value,
        ),
        property(
            "Data size",
            &format_bytes(dataset_info.data_size_bytes),
            label,
            value,
        ),
        property(
            "Deletion files",
            &dataset_info.deletion_files.to_string(),
            label,
            value,
        ),
        property(
            "Deleted rows",
            &dataset_info.deleted_rows.to_string(),
            label,
            value,
        ),
        property(
            "Manifest size",
            &optional_bytes(dataset_info.manifest_size_bytes),
            label,
            value,
        ),
        property(
            "Indices",
            &dataset_info.index_count.to_string(),
            label,
            value,
        ),
        property(
            "Index size",
            &format!(
                "{}{}",
                format_bytes(dataset_info.index_size_bytes),
                if dataset_info.index_size_complete {
                    ""
                } else {
                    " (partial)"
                }
            ),
            label,
            value,
        ),
        Line::from(""),
    ];
    lines
}

fn property_label_style(theme: &Theme) -> ratatui::style::Style {
    let label: ratatui::style::Style = theme.style("text.primary").into();
    label.add_modifier(Modifier::BOLD)
}

fn schema_table(dataset_info: &DatasetInfo, theme: &Theme) -> Table<'static> {
    let header = Row::new([
        Cell::from("ID"),
        Cell::from("Field"),
        Cell::from("Type"),
        Cell::from("Nullability"),
        Cell::from("Keys"),
    ])
    .style(Style::default().fg(theme.color("accent.primary").into()));
    let rows = dataset_info.fields.iter().map(|field| {
        let nullability = if field.nullable {
            "nullable"
        } else {
            "required"
        };
        Row::new([
            Cell::from(field.id.to_string()),
            Cell::from(field.path.clone()),
            Cell::from(top_level_type(&field.data_type).to_string()),
            Cell::from(nullability),
            Cell::from(if field.keys.is_empty() {
                "—".to_string()
            } else {
                field.keys.clone()
            }),
        ])
    });

    Table::new(rows, SCHEMA_COLUMN_WIDTHS)
        .header(header.bottom_margin(1))
        .column_spacing(1)
        .style(super::pane_style(theme))
}

fn top_level_type(data_type: &str) -> &str {
    data_type
        .split_once('(')
        .map_or(data_type, |(type_name, _)| type_name)
}

fn property(
    name: &str,
    value: &str,
    label: ratatui::style::Style,
    value_style: ratatui::style::Style,
) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{name:<16}"), label),
        Span::styled(value.to_string(), value_style),
    ])
}

fn optional_bytes(bytes: Option<u64>) -> String {
    bytes.map_or_else(|| "unknown".to_string(), format_bytes)
}

#[cfg(test)]
mod tests {
    use super::top_level_type;

    #[test]
    fn extracts_top_level_type() {
        assert_eq!(top_level_type("List(Field { name: item })"), "List");
        assert_eq!(top_level_type("Int32"), "Int32");
    }
}
