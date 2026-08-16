use crate::{
    app::{DatasetState, TablesFocus},
    dataset::{DatasetInfo, FieldInfo, FragmentInfo, IndexInfo, ManifestInfo},
    formatting::format_bytes,
};
use opaline::Theme;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Cell, Paragraph, Row, Table, TableState, Tabs},
};

use super::{tab_content_block, tab_title_style};

pub(crate) const TITLE: &str = "Tables";
pub(crate) const CONTENT: &str = "No version metadata loaded.";

const MANIFEST_COLUMN_WIDTH: usize = 16;
const VERSIONS_TITLE: &str = "Versions";
const DETAIL_TITLES: [&str; 4] = ["Manifest", "Schema", "Fragments", "Indices"];
const VERSION_COLUMN_WIDTHS: [Constraint; 2] = [Constraint::Length(2), Constraint::Min(0)];
const SCHEMA_COLUMN_WIDTHS: [Constraint; 6] = [
    Constraint::Length(2),
    Constraint::Percentage(8),
    Constraint::Percentage(32),
    Constraint::Percentage(25),
    Constraint::Percentage(18),
    Constraint::Percentage(17),
];

#[derive(Clone, Copy)]
/// Internal representation of RenderState.
pub(crate) struct RenderState {
    pub(crate) selected_version: Option<u64>,
    pub(crate) tables_focus: TablesFocus,
    pub(crate) tables_detail_selected: usize,
    pub(crate) selected_field: Option<usize>,
    pub(crate) selected_fragment: Option<usize>,
    pub(crate) selected_index: Option<usize>,
}

/// Internal helper for render.
pub(crate) fn render(
    frame: &mut Frame,
    area: Rect,
    theme: &Theme,
    dataset_state: &DatasetState,
    render_state: RenderState,
) {
    match dataset_state {
        DatasetState::Loaded(dataset_info) => {
            render_loaded(frame, area, theme, dataset_info, render_state)
        }
        DatasetState::Unavailable { uri, reason } => {
            let lines = vec![
                ratatui::text::Line::styled("Unable to load versions", theme.style("keyword")),
                ratatui::text::Line::from(format!("URI: {uri}")),
                ratatui::text::Line::from(""),
                ratatui::text::Line::from(reason.clone()),
            ];
            render_text(frame, area, theme, lines);
        }
        DatasetState::Loading => render_text(
            frame,
            area,
            theme,
            vec![ratatui::text::Line::from("Loading dataset...")],
        ),
        DatasetState::Empty => {
            render_text(frame, area, theme, vec![ratatui::text::Line::from(CONTENT)])
        }
    }
}

fn render_loaded(
    frame: &mut Frame,
    area: Rect,
    theme: &Theme,
    dataset_info: &DatasetInfo,
    render_state: RenderState,
) {
    let [tables_area, right_area] =
        Layout::horizontal([Constraint::Percentage(10), Constraint::Percentage(90)])
            .spacing(1)
            .areas(area);
    let detail_block = tab_content_block(theme)
        .title_style(tab_title_style(theme))
        .border_style(
            if render_state.selected_version.is_some()
                && render_state.tables_focus == TablesFocus::Details
            {
                theme.style("focused_border")
            } else {
                theme.style("unfocused_border")
            },
        )
        .title("Version Details");
    let detail_inner = detail_block.inner(right_area);
    frame.render_widget(detail_block, right_area);
    let [detail_tabs_area, detail_area] =
        Layout::vertical([Constraint::Length(1), Constraint::Min(0)])
            .spacing(1)
            .areas(detail_inner);

    render_versions(
        frame,
        tables_area,
        theme,
        dataset_info,
        render_state.selected_version,
        render_state.tables_focus,
    );
    render_detail_tabs(
        frame,
        detail_tabs_area,
        theme,
        render_state.tables_detail_selected,
    );
    match render_state.tables_detail_selected {
        0 => render_manifest(
            frame,
            detail_area,
            theme,
            dataset_info,
            render_state.selected_version,
        ),
        1 => render_schema(
            frame,
            detail_area,
            theme,
            dataset_info,
            render_state.selected_version,
            render_state.selected_field,
        ),
        2 => render_fragments(
            frame,
            detail_area,
            theme,
            dataset_info,
            render_state.selected_version,
            render_state.selected_fragment,
        ),
        3 => render_indices(
            frame,
            detail_area,
            theme,
            dataset_info,
            render_state.selected_version,
            render_state.selected_index,
        ),
        _ => unreachable!("table detail tab index is bounded by AppState"),
    }
}

fn render_indices(
    frame: &mut Frame,
    area: Rect,
    theme: &Theme,
    dataset_info: &DatasetInfo,
    selected_version: Option<u64>,
    selected_index: Option<usize>,
) {
    let version = selected_version.unwrap_or(dataset_info.current_version);
    match dataset_info.index_cache.get(&version) {
        Some(Ok(indices)) if indices.is_empty() => frame.render_widget(
            Paragraph::new("No indices for this version.").style(super::pane_style(theme)),
            area,
        ),
        Some(Ok(indices)) => {
            let header = Row::new([
                Cell::from("Name"),
                Cell::from("Type"),
                Cell::from("Fields"),
                Cell::from("Files"),
                Cell::from("Size"),
            ])
            .style(Style::default().fg(theme.color("accent.primary").into()))
            .bottom_margin(1);
            let selected_index = selected_index.or_else(|| (!indices.is_empty()).then_some(0));
            let rows = indices.iter().enumerate().map(|(row_index, index)| {
                index_row(
                    index,
                    row_index,
                    selected_index,
                    dataset_info,
                    version,
                    theme,
                )
            });
            let mut table_state = TableState::default();
            table_state.select(selected_index);
            frame.render_stateful_widget(
                Table::new(
                    rows,
                    [
                        Constraint::Length(2),
                        Constraint::Percentage(20),
                        Constraint::Percentage(15),
                        Constraint::Percentage(35),
                        Constraint::Percentage(10),
                        Constraint::Percentage(20),
                    ],
                )
                .header(header)
                .column_spacing(1)
                .style(super::pane_style(theme)),
                area,
                &mut table_state,
            );
        }
        Some(Err(reason)) => frame.render_widget(
            Paragraph::new(vec![
                Line::styled("Unable to load indices", theme.style("keyword")),
                Line::from(format!("Version: {version}")),
                Line::from(""),
                Line::from(reason.clone()),
            ])
            .style(super::pane_style(theme)),
            area,
        ),
        None => frame.render_widget(
            Paragraph::new("Index metadata not loaded.").style(super::pane_style(theme)),
            area,
        ),
    }
}

fn index_row(
    index: &IndexInfo,
    row_index: usize,
    selected_index: Option<usize>,
    dataset_info: &DatasetInfo,
    version: u64,
    theme: &Theme,
) -> Row<'static> {
    let mut row = Row::new([
        Cell::from(if selected_index == Some(row_index) {
            "▸"
        } else {
            ""
        }),
        Cell::from(index.name.clone()),
        Cell::from(index.index_type.as_deref().unwrap_or("unknown").to_string()),
        Cell::from(super::indices::index_field_names(
            index,
            super::indices::fields_for_version(dataset_info, version),
        )),
        Cell::from(
            index
                .files
                .as_ref()
                .map_or_else(|| "unknown".to_string(), |files| files.len().to_string()),
        ),
        Cell::from(
            index
                .total_size_bytes
                .map_or_else(|| "unknown".to_string(), format_bytes),
        ),
    ]);
    if selected_index == Some(row_index) {
        row = row.style(
            super::pane_style(theme)
                .bg(theme.color("bg.active").into())
                .fg(theme.color("accent.primary").into())
                .add_modifier(Modifier::BOLD),
        );
    }
    row
}

fn render_detail_tabs(frame: &mut Frame, area: Rect, theme: &Theme, selected_detail: usize) {
    frame.render_widget(
        Tabs::new(DETAIL_TITLES)
            .select(selected_detail)
            .style(theme.style("text.secondary"))
            .highlight_style(
                Style::default()
                    .fg(theme.color("accent.primary").into())
                    .bg(theme.color("bg.active").into())
                    .add_modifier(Modifier::BOLD),
            ),
        area,
    );
}

fn render_manifest(
    frame: &mut Frame,
    area: Rect,
    theme: &Theme,
    dataset_info: &DatasetInfo,
    selected_version: Option<u64>,
) {
    let version = selected_version.unwrap_or(dataset_info.current_version);
    let lines = match dataset_info.manifest_cache.get(&version) {
        Some(Ok(manifest)) => manifest_lines(manifest, theme),
        Some(Err(reason)) => vec![
            Line::styled("Unable to load manifest", theme.style("keyword")),
            Line::from(format!("Version: {version}")),
            Line::from(""),
            Line::from(reason.clone()),
        ],
        None => vec![Line::from("Manifest metadata not loaded.")],
    };

    frame.render_widget(Paragraph::new(lines).style(super::pane_style(theme)), area);
}

fn manifest_lines(manifest: &ManifestInfo, theme: &Theme) -> Vec<Line<'static>> {
    let label: Style = theme.style("text.primary").into();
    let label = label.add_modifier(Modifier::BOLD);
    let value: Style = theme.style("text.primary").into();
    vec![
        property("Path", &manifest.path, label, value),
        property("Version", &manifest.version.to_string(), label, value),
        property("Committed", &manifest.timestamp, label, value),
        property(
            "Size",
            &manifest
                .size_bytes
                .map_or_else(|| "unknown".to_string(), format_bytes),
            label,
            value,
        ),
        property("Writer", &manifest.writer_version, label, value),
        property("Storage format", &manifest.storage_format, label, value),
        property("Branch", &manifest.branch, label, value),
        property("Tag", &manifest.tag, label, value),
    ]
}

fn render_versions(
    frame: &mut Frame,
    area: Rect,
    theme: &Theme,
    dataset_info: &DatasetInfo,
    selected_version: Option<u64>,
    tables_focus: TablesFocus,
) {
    let effective_selected = selected_version.or(Some(dataset_info.current_version));
    let block = tab_content_block(theme)
        .title_style(tab_title_style(theme))
        .border_style(
            if selected_version.is_some() && tables_focus == TablesFocus::Versions {
                theme.style("focused_border")
            } else {
                theme.style("unfocused_border")
            },
        )
        .title(VERSIONS_TITLE);
    let table_area = block.inner(area);
    frame.render_widget(block, area);

    let header = Row::new([Cell::from(""), Cell::from("Version")])
        .style(Style::default().fg(theme.color("accent.primary").into()))
        .bottom_margin(1);
    let rows = dataset_info.versions.iter().rev().map(|version| {
        let marked = effective_selected == Some(version.version);
        let mut row = Row::new([
            Cell::from(if marked { "▸" } else { "" }),
            Cell::from(version.version.to_string()),
        ]);
        let mut row_style = super::pane_style(theme);
        if effective_selected == Some(version.version) {
            row_style = row_style
                .bg(theme.color("bg.active").into())
                .fg(theme.color("accent.primary").into())
                .add_modifier(Modifier::BOLD);
        }
        row = row.style(row_style);
        row
    });

    let selected_index = effective_selected.and_then(|selected| {
        dataset_info
            .versions
            .iter()
            .rev()
            .position(|version| version.version == selected)
    });
    let mut table_state = TableState::default();
    table_state.select(selected_index);
    frame.render_stateful_widget(
        Table::new(rows, VERSION_COLUMN_WIDTHS)
            .header(header)
            .column_spacing(1)
            .style(super::pane_style(theme)),
        table_area,
        &mut table_state,
    );
}

fn render_schema(
    frame: &mut Frame,
    area: Rect,
    theme: &Theme,
    dataset_info: &DatasetInfo,
    selected_version: Option<u64>,
    selected_field: Option<usize>,
) {
    let version = selected_version.unwrap_or(dataset_info.current_version);
    match dataset_info.schema_cache.get(&version) {
        Some(Ok(fields)) => {
            let details = fields
                .get(selected_field.unwrap_or(0))
                .map(|field| field_details(field, theme))
                .unwrap_or_default();
            let [table_area, details_area] =
                Layout::vertical([Constraint::Min(1), Constraint::Length(details.len() as u16)])
                    .spacing(1)
                    .areas(area);
            let selected_index = selected_field.or_else(|| (!fields.is_empty()).then_some(0));
            let mut table_state = TableState::default();
            table_state.select(selected_index);
            frame.render_stateful_widget(
                schema_table(fields, theme, selected_field),
                table_area,
                &mut table_state,
            );
            if !details.is_empty() {
                frame.render_widget(
                    Paragraph::new(details).style(super::pane_style(theme)),
                    details_area,
                );
            }
        }
        Some(Err(reason)) => frame.render_widget(
            Paragraph::new(vec![
                Line::styled("Unable to load schema", theme.style("keyword")),
                Line::from(format!("Version: {version}")),
                Line::from(""),
                Line::from(reason.clone()),
            ])
            .style(super::pane_style(theme)),
            area,
        ),
        None => frame.render_widget(
            Paragraph::new("Schema metadata not loaded.").style(super::pane_style(theme)),
            area,
        ),
    }
}

fn render_fragments(
    frame: &mut Frame,
    area: Rect,
    theme: &Theme,
    dataset_info: &DatasetInfo,
    selected_version: Option<u64>,
    selected_fragment: Option<usize>,
) {
    let version = selected_version.unwrap_or(dataset_info.current_version);
    match dataset_info.manifest_cache.get(&version) {
        Some(Ok(manifest)) => {
            let details = manifest
                .fragments
                .get(selected_fragment.unwrap_or(0))
                .map(|fragment| fragment_details(fragment, theme))
                .unwrap_or_default();
            let [table_area, details_area] =
                Layout::vertical([Constraint::Min(1), Constraint::Length(details.len() as u16)])
                    .spacing(1)
                    .areas(area);
            let header = Row::new([
                Cell::from(""),
                Cell::from("ID"),
                Cell::from("Rows"),
                Cell::from("Physical Rows"),
                Cell::from("Files"),
                Cell::from("Deleted Rows"),
            ])
            .style(Style::default().fg(theme.color("accent.primary").into()))
            .bottom_margin(1);
            let rows = manifest
                .fragments
                .iter()
                .enumerate()
                .map(|(index, fragment)| fragment_row(fragment, index, selected_fragment, theme));
            let selected_index =
                selected_fragment.or_else(|| (!manifest.fragments.is_empty()).then_some(0));
            let mut table_state = TableState::default();
            table_state.select(selected_index);
            frame.render_stateful_widget(
                Table::new(
                    rows,
                    [
                        Constraint::Length(2),
                        Constraint::Percentage(12),
                        Constraint::Percentage(22),
                        Constraint::Percentage(22),
                        Constraint::Percentage(18),
                        Constraint::Percentage(26),
                    ],
                )
                .header(header)
                .column_spacing(1)
                .style(super::pane_style(theme)),
                table_area,
                &mut table_state,
            );
            if !details.is_empty() {
                frame.render_widget(
                    Paragraph::new(details).style(super::pane_style(theme)),
                    details_area,
                );
            }
        }
        Some(Err(reason)) => frame.render_widget(
            Paragraph::new(vec![
                Line::styled("Unable to load fragments", theme.style("keyword")),
                Line::from(format!("Version: {version}")),
                Line::from(""),
                Line::from(reason.clone()),
            ])
            .style(super::pane_style(theme)),
            area,
        ),
        None => frame.render_widget(
            Paragraph::new("Fragment metadata not loaded.").style(super::pane_style(theme)),
            area,
        ),
    }
}

fn fragment_row(
    fragment: &FragmentInfo,
    index: usize,
    selected_fragment: Option<usize>,
    theme: &Theme,
) -> Row<'static> {
    let marked = selected_fragment == Some(index) || (selected_fragment.is_none() && index == 0);
    let mut row = Row::new([
        Cell::from(if marked { "▸" } else { "" }),
        Cell::from(fragment.id.to_string()),
        Cell::from(format_count(fragment.rows)),
        Cell::from(format_count(fragment.physical_rows)),
        Cell::from(fragment.data_files.to_string()),
        Cell::from(format_count(fragment.deleted_rows)),
    ]);
    if selected_fragment == Some(index) {
        row = row.style(
            super::pane_style(theme)
                .bg(theme.color("bg.active").into())
                .fg(theme.color("accent.primary").into())
                .add_modifier(Modifier::BOLD),
        );
    }
    row
}

fn format_count(value: Option<u64>) -> String {
    value.map_or_else(|| "unknown".to_string(), |value| value.to_string())
}

fn fragment_details(fragment: &FragmentInfo, theme: &Theme) -> Vec<Line<'static>> {
    let label: Style = theme.style("text.primary").into();
    let label = label.add_modifier(Modifier::BOLD);
    let value: Style = theme.style("text.primary").into();
    let mut lines = vec![Line::styled("Fragment details", theme.style("keyword"))];
    for (index, data_file) in fragment.data_file_details.iter().enumerate() {
        lines.push(property(
            &format!("Data file {}", index + 1),
            &format!("{} (format {})", data_file.path, data_file.format),
            label,
            value,
        ));
    }
    if let Some(deletion_file) = &fragment.deletion_file {
        lines.push(property(
            "Deletion file",
            &format!(
                "{} ({}, read v{})",
                deletion_file.id, deletion_file.file_type, deletion_file.read_version
            ),
            label,
            value,
        ));
    }
    lines
}

fn schema_table(
    fields: &[FieldInfo],
    theme: &Theme,
    selected_field: Option<usize>,
) -> Table<'static> {
    let header = Row::new([
        Cell::from(""),
        Cell::from("ID"),
        Cell::from("Field"),
        Cell::from("Type"),
        Cell::from("Nullability"),
        Cell::from("Keys"),
    ])
    .style(Style::default().fg(theme.color("accent.primary").into()))
    .bottom_margin(1);
    let rows = fields.iter().enumerate().map(|(index, field)| {
        let marked = selected_field == Some(index) || (selected_field.is_none() && index == 0);
        let nullability = if field.nullable {
            "nullable"
        } else {
            "required"
        };
        let mut row = Row::new([
            Cell::from(if marked { "▸" } else { "" }),
            Cell::from(field.id.to_string()),
            Cell::from(field.path.clone()),
            Cell::from(top_level_type(&field.data_type).to_string()),
            Cell::from(nullability),
            Cell::from(if field.keys.is_empty() {
                "—".to_string()
            } else {
                field.keys.clone()
            }),
        ]);
        if selected_field == Some(index) {
            row = row.style(
                super::pane_style(theme)
                    .bg(theme.color("bg.active").into())
                    .fg(theme.color("accent.primary").into())
                    .add_modifier(Modifier::BOLD),
            );
        }
        row
    });

    Table::new(rows, SCHEMA_COLUMN_WIDTHS)
        .header(header)
        .column_spacing(1)
        .style(super::pane_style(theme))
}

fn field_details(field: &FieldInfo, theme: &Theme) -> Vec<Line<'static>> {
    let label: Style = theme.style("text.primary").into();
    let label = label.add_modifier(Modifier::BOLD);
    let value: Style = theme.style("text.primary").into();
    let mut lines = vec![Line::styled("Field details", theme.style("keyword"))];
    if top_level_type(&field.data_type) != field.data_type.as_str() {
        lines.push(property("Full type", &field.data_type, label, value));
    }
    if field.metadata.is_empty() {
        lines.push(property("Metadata", "none", label, value));
    } else {
        for (key, metadata) in &field.metadata {
            lines.push(property(
                "Metadata",
                &format!("{key}: {metadata}"),
                label,
                value,
            ));
        }
    }
    lines
}

fn top_level_type(data_type: &str) -> &str {
    data_type
        .split_once('(')
        .map_or(data_type, |(type_name, _)| type_name)
}

fn property(name: &str, value: &str, label: Style, value_style: Style) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{name:<MANIFEST_COLUMN_WIDTH$}"), label),
        Span::styled(value.to_string(), value_style),
    ])
}

fn render_text(
    frame: &mut Frame,
    area: Rect,
    theme: &Theme,
    lines: Vec<ratatui::text::Line<'static>>,
) {
    frame.render_widget(
        Paragraph::new(lines).style(super::pane_style(theme)).block(
            tab_content_block(theme)
                .title_style(tab_title_style(theme))
                .title(TITLE),
        ),
        area,
    );
}
