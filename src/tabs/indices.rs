use crate::{
    app::{DatasetState, IndicesFocus},
    dataset::{DatasetInfo, FieldInfo, IndexFileInfo, IndexInfo},
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
use std::collections::{BTreeMap, BTreeSet};

use super::{tab_content_block, tab_title_style};

pub(crate) const TITLE: &str = "Indices";
const VERSIONS_TITLE: &str = "Versions";
const INDICES_TITLE: &str = "Indices";
const DETAILS_TITLE: &str = "Index Details";
const DETAIL_TITLES: [&str; 2] = ["Metadata", "Index Files"];

/// Internal representation of RenderState.
pub(crate) struct RenderState<'a> {
    pub(crate) selected_version: Option<u64>,
    pub(crate) focus: Option<IndicesFocus>,
    pub(crate) selected_index: Option<usize>,
    pub(crate) selected_file: Option<usize>,
    pub(crate) selected_detail: usize,
    pub(crate) layout_outline_expanded: &'a BTreeMap<(u64, String, String), BTreeSet<String>>,
    pub(crate) layout_outline_selected: Option<usize>,
}

struct DetailsState<'a> {
    selected_index: Option<usize>,
    selected_file: Option<usize>,
    selected_detail: usize,
    layout_outline_expanded: &'a BTreeMap<(u64, String, String), BTreeSet<String>>,
    layout_outline_selected: Option<usize>,
}

#[derive(Clone, Copy)]
struct LayoutState<'a> {
    focus: Option<IndicesFocus>,
    expanded: &'a BTreeMap<(u64, String, String), BTreeSet<String>>,
    selected: Option<usize>,
}

struct IndexFileView<'a> {
    dataset_info: &'a DatasetInfo,
    version: u64,
    index: &'a IndexInfo,
    selected_file: Option<usize>,
    layout_state: LayoutState<'a>,
}

/// Internal helper for render.
pub(crate) fn render(
    frame: &mut Frame,
    area: Rect,
    theme: &Theme,
    dataset_state: &DatasetState,
    render_state: RenderState<'_>,
) {
    let [versions_area, indices_area, details_area] = Layout::horizontal([
        Constraint::Percentage(10),
        Constraint::Percentage(20),
        Constraint::Percentage(70),
    ])
    .spacing(1)
    .areas(area);

    match dataset_state {
        DatasetState::Loaded(dataset_info) => {
            render_versions(
                frame,
                versions_area,
                theme,
                dataset_info,
                render_state.selected_version,
                render_state.focus,
            );
            render_indices(
                frame,
                indices_area,
                theme,
                dataset_info,
                render_state.selected_version,
                render_state.focus,
                render_state.selected_index,
            );
            render_details(
                frame,
                details_area,
                theme,
                dataset_info,
                render_state.selected_version,
                render_state.focus,
                DetailsState {
                    selected_index: render_state.selected_index,
                    selected_file: render_state.selected_file,
                    selected_detail: render_state.selected_detail,
                    layout_outline_expanded: render_state.layout_outline_expanded,
                    layout_outline_selected: render_state.layout_outline_selected,
                },
            );
        }
        DatasetState::Unavailable { uri, reason } => {
            render_placeholder(
                frame,
                versions_area,
                theme,
                VERSIONS_TITLE,
                false,
                vec![
                    Line::styled("Unable to load versions", theme.style("keyword")),
                    Line::from(format!("URI: {uri}")),
                    Line::from(""),
                    Line::from(reason.clone()),
                ],
            );
            render_placeholder(
                frame,
                indices_area,
                theme,
                INDICES_TITLE,
                false,
                vec![Line::from("No indices loaded.")],
            );
            render_placeholder(
                frame,
                details_area,
                theme,
                DETAILS_TITLE,
                false,
                vec![Line::from("No index details loaded.")],
            );
        }
        DatasetState::Loading => {
            render_placeholder(
                frame,
                versions_area,
                theme,
                "Versions",
                render_state.focus == Some(IndicesFocus::Versions),
                vec![Line::from("Loading dataset...")],
            );
            render_placeholder(
                frame,
                indices_area,
                theme,
                "Indices",
                render_state.focus == Some(IndicesFocus::Indices),
                vec![Line::from("Loading dataset...")],
            );
            render_placeholder(
                frame,
                details_area,
                theme,
                "Index Details",
                render_state.focus == Some(IndicesFocus::Details),
                vec![Line::from("Loading dataset...")],
            );
        }
        DatasetState::Empty => {
            render_placeholder(
                frame,
                versions_area,
                theme,
                VERSIONS_TITLE,
                false,
                vec![Line::from("No versions loaded.")],
            );
            render_placeholder(
                frame,
                indices_area,
                theme,
                INDICES_TITLE,
                false,
                vec![Line::from("No indices loaded.")],
            );
            render_placeholder(
                frame,
                details_area,
                theme,
                DETAILS_TITLE,
                false,
                vec![Line::from("No index details loaded.")],
            );
        }
    }
}

fn render_versions(
    frame: &mut Frame,
    area: Rect,
    theme: &Theme,
    dataset_info: &DatasetInfo,
    selected_version: Option<u64>,
    focus: Option<IndicesFocus>,
) {
    let effective_selected = selected_version.or(Some(dataset_info.current_version));
    let block = tab_content_block(theme)
        .title_style(tab_title_style(theme))
        .border_style(if focus == Some(IndicesFocus::Versions) {
            theme.style("focused_border")
        } else {
            theme.style("unfocused_border")
        })
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
        if effective_selected == Some(version.version) {
            row = row.style(selected_row_style(theme));
        }
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
        Table::new(rows, [Constraint::Length(2), Constraint::Min(0)])
            .header(header)
            .column_spacing(1)
            .style(super::pane_style(theme)),
        table_area,
        &mut table_state,
    );
}

fn render_indices(
    frame: &mut Frame,
    area: Rect,
    theme: &Theme,
    dataset_info: &DatasetInfo,
    selected_version: Option<u64>,
    focus: Option<IndicesFocus>,
    selected_index: Option<usize>,
) {
    let version = selected_version.unwrap_or(dataset_info.current_version);
    let block = tab_content_block(theme)
        .title_style(tab_title_style(theme))
        .border_style(if focus == Some(IndicesFocus::Indices) {
            theme.style("focused_border")
        } else {
            theme.style("unfocused_border")
        })
        .title(INDICES_TITLE);
    let table_area = block.inner(area);
    frame.render_widget(block, area);

    match dataset_info.index_cache.get(&version) {
        Some(Ok(indices)) => {
            let row_count = indices.len();
            let selected_index = selected_index
                .filter(|selected| *selected < row_count)
                .or_else(|| (row_count > 0).then_some(0));
            let rows = index_rows(indices, selected_index, theme);
            let header = Row::new([Cell::from(""), Cell::from("Name")])
                .style(Style::default().fg(theme.color("accent.primary").into()))
                .bottom_margin(1);
            let mut table_state = TableState::default();
            table_state.select(selected_index);
            frame.render_stateful_widget(
                Table::new(rows, [Constraint::Length(2), Constraint::Min(0)])
                    .header(header)
                    .column_spacing(1)
                    .style(super::pane_style(theme)),
                table_area,
                &mut table_state,
            );
        }
        Some(Err(reason)) => render_message(
            frame,
            table_area,
            theme,
            vec![
                Line::styled("Unable to load indices", theme.style("keyword")),
                Line::from(format!("Version: {version}")),
                Line::from(""),
                Line::from(reason.clone()),
            ],
        ),
        None => render_message(
            frame,
            table_area,
            theme,
            vec![Line::from("Index metadata not loaded.")],
        ),
    }
}

fn render_details(
    frame: &mut Frame,
    area: Rect,
    theme: &Theme,
    dataset_info: &DatasetInfo,
    selected_version: Option<u64>,
    focus: Option<IndicesFocus>,
    details_state: DetailsState<'_>,
) {
    let version = selected_version.unwrap_or(dataset_info.current_version);
    let block = tab_content_block(theme)
        .title_style(tab_title_style(theme))
        .border_style(if focus == Some(IndicesFocus::Details) {
            theme.style("focused_border")
        } else {
            theme.style("unfocused_border")
        })
        .title(DETAILS_TITLE);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let [tabs_area, detail_area] = Layout::vertical([Constraint::Length(1), Constraint::Min(0)])
        .spacing(1)
        .areas(inner);
    render_detail_tabs(frame, tabs_area, theme, details_state.selected_detail);

    match dataset_info.index_cache.get(&version) {
        Some(Ok(indices)) => {
            let selected_index = details_state
                .selected_index
                .filter(|selected| *selected < indices.len())
                .or_else(|| (!indices.is_empty()).then_some(0));
            let Some(index) = selected_index.and_then(|selected| indices.get(selected)) else {
                return render_message(
                    frame,
                    detail_area,
                    theme,
                    vec![Line::from("No index selected.")],
                );
            };
            match details_state.selected_detail {
                0 => render_metadata(frame, detail_area, theme, index, dataset_info, version),
                1 => render_index_files(
                    frame,
                    detail_area,
                    theme,
                    IndexFileView {
                        dataset_info,
                        version,
                        index,
                        selected_file: details_state.selected_file,
                        layout_state: LayoutState {
                            focus,
                            expanded: details_state.layout_outline_expanded,
                            selected: details_state.layout_outline_selected,
                        },
                    },
                ),
                _ => unreachable!("index detail tab index is bounded by AppState"),
            }
        }
        Some(Err(reason)) => render_message(
            frame,
            detail_area,
            theme,
            vec![
                Line::styled("Unable to load index details", theme.style("keyword")),
                Line::from(format!("Version: {version}")),
                Line::from(""),
                Line::from(reason.clone()),
            ],
        ),
        None => render_message(
            frame,
            detail_area,
            theme,
            vec![Line::from("Index metadata not loaded.")],
        ),
    }
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

fn render_metadata(
    frame: &mut Frame,
    area: Rect,
    theme: &Theme,
    index: &IndexInfo,
    dataset_info: &DatasetInfo,
    version: u64,
) {
    frame.render_widget(
        Paragraph::new(index_metadata_lines(
            index,
            fields_for_version(dataset_info, version),
            theme,
        ))
        .style(super::pane_style(theme)),
        area,
    );
}

fn render_index_files(frame: &mut Frame, area: Rect, theme: &Theme, view: IndexFileView<'_>) {
    let [file_list_area, file_layout_area] =
        Layout::vertical([Constraint::Percentage(20), Constraint::Min(0)])
            .spacing(1)
            .areas(area);
    let Some(files) = view.index.files.as_ref() else {
        render_message(
            frame,
            file_list_area,
            theme,
            vec![Line::from("Index files not available.")],
        );
        return render_file_layout(frame, file_layout_area, theme, view);
    };
    if files.is_empty() {
        render_message(
            frame,
            file_list_area,
            theme,
            vec![Line::from("No index files.")],
        );
        return render_file_layout(frame, file_layout_area, theme, view);
    }

    let header = Row::new([Cell::from(""), Cell::from("Path"), Cell::from("Size")])
        .style(Style::default().fg(theme.color("accent.primary").into()))
        .bottom_margin(1);
    let selected_file = view
        .selected_file
        .filter(|selected| *selected < files.len());
    let rows = files
        .iter()
        .enumerate()
        .map(|(row_index, file)| index_file_row(file, row_index, selected_file, theme));
    let mut table_state = TableState::default();
    table_state.select(selected_file);
    frame.render_stateful_widget(
        Table::new(
            rows,
            [
                Constraint::Length(2),
                Constraint::Min(0),
                Constraint::Length(12),
            ],
        )
        .header(header)
        .column_spacing(1)
        .style(super::pane_style(theme)),
        file_list_area,
        &mut table_state,
    );
    render_file_layout(frame, file_layout_area, theme, view);
}

fn index_file_row(
    file: &IndexFileInfo,
    row_index: usize,
    selected_file: Option<usize>,
    theme: &Theme,
) -> Row<'static> {
    let mut row = Row::new([
        Cell::from(if selected_file == Some(row_index) {
            "▸"
        } else {
            ""
        }),
        Cell::from(file.path.clone()),
        Cell::from(format_bytes(file.size_bytes)),
    ]);
    if selected_file == Some(row_index) {
        row = row.style(selected_row_style(theme));
    }
    row
}

fn render_file_layout(frame: &mut Frame, area: Rect, theme: &Theme, view: IndexFileView<'_>) {
    let [outline_area, file_layout_area] =
        Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
            .spacing(1)
            .areas(area);
    let Some(files) = view.index.files.as_ref() else {
        render_message(
            frame,
            outline_area,
            theme,
            vec![Line::from("Layout Outline not available.")],
        );
        return render_message(
            frame,
            file_layout_area,
            theme,
            vec![Line::from("Index files not available.")],
        );
    };
    let Some(file) = view.selected_file.and_then(|selected| files.get(selected)) else {
        render_message(
            frame,
            outline_area,
            theme,
            vec![Line::from("No index file selected.")],
        );
        return render_message(
            frame,
            file_layout_area,
            theme,
            vec![Line::from("No index file selected.")],
        );
    };

    let cache_key = (view.version, view.index.uuid.clone(), file.path.clone());
    match view.dataset_info.index_file_layout_cache.get(&cache_key) {
        Some(Ok(layout)) => {
            let selected_outline_key = super::data_files::render_layout_outline(
                frame,
                outline_area,
                theme,
                layout,
                view.layout_state.expanded.get(&cache_key),
                view.layout_state.selected,
                view.layout_state.focus == Some(IndicesFocus::LayoutOutline),
            );
            super::data_files::render_file_layout_map(
                frame,
                file_layout_area,
                theme,
                layout,
                selected_outline_key.as_deref(),
            );
        }
        Some(Err(reason)) => {
            render_message(
                frame,
                outline_area,
                theme,
                vec![Line::from("Layout Outline unavailable.")],
            );
            render_message(
                frame,
                file_layout_area,
                theme,
                vec![
                    Line::styled("Unable to parse index file", theme.style("keyword")),
                    Line::from(format!("Path: {}", file.path)),
                    Line::from(""),
                    Line::from(reason.clone()),
                ],
            );
        }
        None => {
            render_message(
                frame,
                outline_area,
                theme,
                vec![Line::from("Layout Outline is loading...")],
            );
            render_message(
                frame,
                file_layout_area,
                theme,
                vec![
                    Line::from(format!("Path: {}", file.path)),
                    Line::from(format!("Size: {}", format_bytes(file.size_bytes))),
                    Line::from("Loading binary layout..."),
                ],
            );
        }
    }
}

fn index_metadata_lines(
    index: &IndexInfo,
    fields: &[FieldInfo],
    theme: &Theme,
) -> Vec<Line<'static>> {
    let label: Style = theme.style("text.primary").into();
    let label = label.add_modifier(Modifier::BOLD);
    let value: Style = theme.style("text.primary").into();
    let mut lines = vec![Line::styled("Index metadata", theme.style("keyword"))];
    lines.push(detail_property("Name", &index.name, label, value));
    lines.push(detail_property(
        "Fields",
        &index_field_names(index, fields),
        label,
        value,
    ));
    lines.push(detail_property(
        "Type",
        index.index_type.as_deref().unwrap_or("unknown"),
        label,
        value,
    ));
    lines.push(detail_property(
        "Indexed rows",
        &index
            .indexed_rows
            .map_or_else(|| "unknown".to_string(), |rows| rows.to_string()),
        label,
        value,
    ));
    lines.push(detail_property(
        "Dataset ver.",
        &index.dataset_version.to_string(),
        label,
        value,
    ));
    lines.push(detail_property(
        "Index ver.",
        &index.index_version.to_string(),
        label,
        value,
    ));
    lines.push(detail_property(
        "Fragments",
        &index
            .fragment_count
            .map_or_else(|| "unknown".to_string(), |count| count.to_string()),
        label,
        value,
    ));
    lines.push(detail_property(
        "Segments",
        &index
            .segment_count
            .map_or_else(|| "unknown".to_string(), |count| count.to_string()),
        label,
        value,
    ));
    lines.push(detail_property(
        "Total size",
        &index
            .total_size_bytes
            .map_or_else(|| "unknown".to_string(), format_bytes),
        label,
        value,
    ));
    lines
}

/// Internal helper for fields for version.
pub(crate) fn fields_for_version(dataset_info: &DatasetInfo, version: u64) -> &[FieldInfo] {
    dataset_info
        .schema_cache
        .get(&version)
        .and_then(|fields| fields.as_ref().ok())
        .map(Vec::as_slice)
        .unwrap_or_else(|| {
            if version == dataset_info.current_version {
                &dataset_info.fields
            } else {
                &[]
            }
        })
}

/// Internal helper for index field names.
pub(crate) fn index_field_names(index: &IndexInfo, fields: &[FieldInfo]) -> String {
    if index.fields.is_empty() {
        return "—".to_string();
    }
    index
        .fields
        .iter()
        .map(|field_id| {
            fields
                .iter()
                .find(|field| field.id == *field_id)
                .map_or_else(|| format!("field {field_id}"), |field| field.path.clone())
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn detail_property(name: &str, value: &str, label: Style, value_style: Style) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{name:<16}"), label),
        Span::styled(value.to_string(), value_style),
    ])
}

fn index_rows(
    indices: &[IndexInfo],
    selected_index: Option<usize>,
    theme: &Theme,
) -> Vec<Row<'static>> {
    indices
        .iter()
        .enumerate()
        .map(|(row_index, index)| index_row(index, row_index, selected_index, theme))
        .collect()
}

fn index_row(
    index: &IndexInfo,
    row_index: usize,
    selected_index: Option<usize>,
    theme: &Theme,
) -> Row<'static> {
    let mut row = Row::new([
        Cell::from(if selected_index == Some(row_index) {
            "▸"
        } else {
            ""
        }),
        Cell::from(index.name.clone()),
    ]);
    if selected_index == Some(row_index) {
        row = row.style(selected_row_style(theme));
    }
    row
}

fn selected_row_style(theme: &Theme) -> Style {
    super::pane_style(theme)
        .bg(theme.color("bg.active").into())
        .fg(theme.color("accent.primary").into())
        .add_modifier(Modifier::BOLD)
}

fn render_placeholder(
    frame: &mut Frame,
    area: Rect,
    theme: &Theme,
    title: &str,
    focused: bool,
    lines: Vec<Line<'static>>,
) {
    let block = tab_content_block(theme)
        .title_style(tab_title_style(theme))
        .border_style(if focused {
            theme.style("focused_border")
        } else {
            theme.style("unfocused_border")
        })
        .title(title);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    render_message(frame, inner, theme, lines);
}

fn render_message(frame: &mut Frame, area: Rect, theme: &Theme, lines: Vec<Line<'static>>) {
    frame.render_widget(Paragraph::new(lines).style(super::pane_style(theme)), area);
}

#[cfg(test)]
mod tests {
    use super::index_field_names;
    use crate::dataset::{FieldInfo, IndexInfo};

    #[test]
    fn resolves_index_fields_from_the_selected_version_schema() {
        let index = IndexInfo {
            uuid: "index".to_string(),
            base_id: None,
            name: "by_name".to_string(),
            dataset_version: 1,
            fields: vec![7],
            index_version: 1,
            fragment_count: None,
            files: None,
            index_type: None,
            indexed_rows: None,
            segment_count: None,
            total_size_bytes: None,
        };
        let historical_fields = vec![FieldInfo {
            id: 7,
            path: "previous_name".to_string(),
            data_type: "Utf8".to_string(),
            nullable: false,
            keys: String::new(),
            metadata: Vec::new(),
        }];

        assert_eq!(
            index_field_names(&index, &historical_fields),
            "previous_name"
        );
    }
}
