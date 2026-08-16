use std::collections::{BTreeMap, BTreeSet};

use crate::dataset::{
    ByteRange, DataFileInfo, DatasetInfo, FileLayoutInfo, FileLayoutKind, FragmentInfo,
    LegacyFileLayout, V2FileLayout, layout_outline_tree,
};
use crate::{
    app::{DataFilesFocus, DatasetState},
    formatting::format_bytes,
};
use opaline::{Theme, ThemeVariant};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Cell, Clear, Paragraph, Row, Table, TableState},
};

use super::{tab_content_block, tab_title_style};

pub(crate) const TITLE: &str = "Data Files";
const COLUMN_TITLES: [&str; 3] = ["Versions", "Fragments", "Data Files"];
const COLUMN_CONTENT: [&str; 2] = ["No fragments loaded.", "No data files loaded."];
const INNER_TITLES: [&str; 2] = ["Layout Outline", "File Layout"];
const VERSION_COLUMN_WIDTHS: [Constraint; 2] = [Constraint::Length(2), Constraint::Min(0)];

pub(crate) struct RenderState<'a> {
    pub(crate) dataset_state: DatasetState,
    pub(crate) selected_version: Option<u64>,
    pub(crate) selected_fragment: Option<usize>,
    pub(crate) selected_data_file: Option<usize>,
    pub(crate) focus: Option<DataFilesFocus>,
    pub(crate) layout_outline_expanded: &'a BTreeMap<(u64, u64, usize), BTreeSet<String>>,
    pub(crate) layout_outline_selected: Option<usize>,
}

#[derive(Clone, Copy)]
struct SelectionState<'a> {
    selected_version: Option<u64>,
    selected_fragment: Option<usize>,
    selected_data_file: Option<usize>,
    focus: Option<DataFilesFocus>,
    layout_outline_expanded: &'a BTreeMap<(u64, u64, usize), BTreeSet<String>>,
    layout_outline_selected: Option<usize>,
    layout_outline_key: Option<(u64, u64, usize)>,
}

pub(crate) fn render(frame: &mut Frame, area: Rect, theme: &Theme, render_state: RenderState) {
    let [versions_area, fragments_area, data_files_area] = Layout::horizontal([
        Constraint::Percentage(10),
        Constraint::Percentage(10),
        Constraint::Percentage(80),
    ])
    .spacing(1)
    .areas(area);

    match &render_state.dataset_state {
        DatasetState::Loaded(dataset_info) => render_versions(
            frame,
            versions_area,
            theme,
            dataset_info,
            render_state.selected_version,
            render_state.focus,
        ),
        DatasetState::Unavailable { uri, reason } => render_placeholder(
            frame,
            versions_area,
            theme,
            COLUMN_TITLES[0],
            render_state.focus == Some(DataFilesFocus::Versions),
            vec![
                ratatui::text::Line::styled("Unable to load versions", theme.style("keyword")),
                ratatui::text::Line::from(format!("URI: {uri}")),
                ratatui::text::Line::from(""),
                ratatui::text::Line::from(reason.clone()),
            ],
        ),
        DatasetState::Loading => render_placeholder(
            frame,
            versions_area,
            theme,
            COLUMN_TITLES[0],
            false,
            vec![ratatui::text::Line::from("Loading dataset...")],
        ),
        DatasetState::Empty => render_placeholder(
            frame,
            versions_area,
            theme,
            COLUMN_TITLES[0],
            render_state.focus == Some(DataFilesFocus::Versions),
            vec![ratatui::text::Line::from("No versions loaded.")],
        ),
    }

    match &render_state.dataset_state {
        DatasetState::Loaded(dataset_info) => render_fragments(
            frame,
            fragments_area,
            theme,
            dataset_info,
            render_state.selected_version,
            render_state.selected_fragment,
            render_state.focus,
        ),
        DatasetState::Unavailable { .. } | DatasetState::Loading | DatasetState::Empty => {
            render_placeholder(
                frame,
                fragments_area,
                theme,
                COLUMN_TITLES[1],
                render_state.focus == Some(DataFilesFocus::Fragments),
                vec![ratatui::text::Line::from(COLUMN_CONTENT[0])],
            )
        }
    }

    match &render_state.dataset_state {
        DatasetState::Loaded(dataset_info) => render_data_files(
            frame,
            data_files_area,
            theme,
            dataset_info,
            SelectionState {
                selected_version: render_state.selected_version,
                selected_fragment: render_state.selected_fragment,
                selected_data_file: render_state.selected_data_file,
                focus: render_state.focus,
                layout_outline_expanded: render_state.layout_outline_expanded,
                layout_outline_selected: render_state.layout_outline_selected,
                layout_outline_key: None,
            },
        ),
        DatasetState::Unavailable { .. } | DatasetState::Loading | DatasetState::Empty => {
            render_placeholder(
                frame,
                data_files_area,
                theme,
                COLUMN_TITLES[2],
                render_state.focus == Some(DataFilesFocus::DataFiles),
                vec![ratatui::text::Line::from(COLUMN_CONTENT[1])],
            )
        }
    }
}

fn render_versions(
    frame: &mut Frame,
    area: Rect,
    theme: &Theme,
    dataset_info: &DatasetInfo,
    selected_version: Option<u64>,
    data_files_focus: Option<DataFilesFocus>,
) {
    let effective_selected = selected_version.or(Some(dataset_info.current_version));
    let block = tab_content_block(theme)
        .title_style(tab_title_style(theme))
        .border_style(
            if selected_version.is_some() && data_files_focus == Some(DataFilesFocus::Versions) {
                theme.style("focused_border")
            } else {
                theme.style("unfocused_border")
            },
        )
        .title(COLUMN_TITLES[0]);
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

fn render_placeholder(
    frame: &mut Frame,
    area: Rect,
    theme: &Theme,
    title: &str,
    focused: bool,
    content: Vec<ratatui::text::Line<'static>>,
) {
    frame.render_widget(
        Paragraph::new(content)
            .style(super::pane_style(theme))
            .block(
                tab_content_block(theme)
                    .title_style(tab_title_style(theme))
                    .border_style(if focused {
                        theme.style("focused_border")
                    } else {
                        theme.style("unfocused_border")
                    })
                    .title(title),
            ),
        area,
    );
}

fn render_fragments(
    frame: &mut Frame,
    area: Rect,
    theme: &Theme,
    dataset_info: &DatasetInfo,
    selected_version: Option<u64>,
    selected_fragment: Option<usize>,
    data_files_focus: Option<DataFilesFocus>,
) {
    let version = selected_version.unwrap_or(dataset_info.current_version);
    let block = tab_content_block(theme)
        .title_style(tab_title_style(theme))
        .border_style(if data_files_focus == Some(DataFilesFocus::Fragments) {
            theme.style("focused_border")
        } else {
            theme.style("unfocused_border")
        })
        .title(COLUMN_TITLES[1]);

    match dataset_info.manifest_cache.get(&version) {
        Some(Ok(manifest)) => {
            let selected_index = selected_fragment_index(&manifest.fragments, selected_fragment);
            let header = Row::new([Cell::from(""), Cell::from("ID")])
                .style(Style::default().fg(theme.color("accent.primary").into()))
                .bottom_margin(1);
            let rows = manifest
                .fragments
                .iter()
                .enumerate()
                .map(|(index, fragment)| fragment_row(fragment, index, selected_index, theme));
            let table_area = block.inner(area);
            frame.render_widget(block, area);

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
        Some(Err(reason)) => render_error(
            frame,
            area,
            theme,
            version,
            reason,
            data_files_focus == Some(DataFilesFocus::Fragments),
        ),
        None => render_placeholder(
            frame,
            area,
            theme,
            COLUMN_TITLES[1],
            data_files_focus == Some(DataFilesFocus::Fragments),
            vec![ratatui::text::Line::from("Fragment metadata not loaded.")],
        ),
    }
}

fn fragment_row(
    fragment: &FragmentInfo,
    index: usize,
    selected_fragment: Option<usize>,
    theme: &Theme,
) -> Row<'static> {
    let marked = selected_fragment == Some(index);
    let mut row = Row::new([
        Cell::from(if marked { "▸" } else { "" }),
        Cell::from(fragment.id.to_string()),
    ]);
    if marked {
        row = row.style(
            super::pane_style(theme)
                .bg(theme.color("bg.active").into())
                .fg(theme.color("accent.primary").into())
                .add_modifier(Modifier::BOLD),
        );
    }
    row
}

fn render_data_files(
    frame: &mut Frame,
    area: Rect,
    theme: &Theme,
    dataset_info: &DatasetInfo,
    selection: SelectionState,
) {
    let version = selection
        .selected_version
        .unwrap_or(dataset_info.current_version);
    let focused = selection.focus == Some(DataFilesFocus::DataFiles);
    let outline_focused = selection.focus == Some(DataFilesFocus::LayoutOutline);
    let block = tab_content_block(theme)
        .title_style(tab_title_style(theme))
        .border_style(if focused {
            theme.style("focused_border")
        } else {
            theme.style("unfocused_border")
        })
        .title(COLUMN_TITLES[2]);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let [file_list_area, inner_area] =
        Layout::vertical([Constraint::Percentage(20), Constraint::Min(0)])
            .spacing(1)
            .areas(inner);
    let [outline_area, file_layout_area] =
        Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
            .spacing(1)
            .areas(inner_area);
    match dataset_info.manifest_cache.get(&version) {
        Some(Ok(manifest)) => {
            let Some(selected_index) =
                selected_fragment_index(&manifest.fragments, selection.selected_fragment)
            else {
                render_layout_placeholders(
                    frame,
                    outline_area,
                    file_layout_area,
                    theme,
                    outline_focused,
                );
                return render_message(
                    frame,
                    file_list_area,
                    theme,
                    vec![ratatui::text::Line::from(COLUMN_CONTENT[1])],
                );
            };
            let fragment = &manifest.fragments[selected_index];

            let header = Row::new([
                Cell::from(""),
                Cell::from("Path"),
                Cell::from("Format"),
                Cell::from("Rows"),
                Cell::from("Columns"),
                Cell::from("Size"),
            ])
            .style(Style::default().fg(theme.color("accent.primary").into()))
            .bottom_margin(1);
            let selected_data_file = selection
                .selected_data_file
                .filter(|index| *index < fragment.data_file_details.len());
            let rows = fragment
                .data_file_details
                .iter()
                .enumerate()
                .map(|(index, data_file)| {
                    data_file_row(data_file, index, selected_data_file, theme)
                });
            frame.render_widget(
                Table::new(
                    rows,
                    [
                        Constraint::Length(2),
                        Constraint::Min(0),
                        Constraint::Length(10),
                        Constraint::Length(10),
                        Constraint::Length(10),
                        Constraint::Length(10),
                    ],
                )
                .header(header)
                .column_spacing(1)
                .style(super::pane_style(theme)),
                file_list_area,
            );

            if let Some(data_file_index) = selected_data_file {
                match dataset_info
                    .file_layout_cache
                    .get(&(version, fragment.id, data_file_index))
                {
                    Some(Ok(layout)) => render_file_layouts(
                        frame,
                        outline_area,
                        file_layout_area,
                        theme,
                        layout,
                        SelectionState {
                            layout_outline_key: Some((version, fragment.id, data_file_index)),
                            ..selection
                        },
                    ),
                    Some(Err(reason)) => render_layout_error(
                        frame,
                        outline_area,
                        file_layout_area,
                        theme,
                        reason,
                        outline_focused,
                    ),
                    None => render_layout_placeholders(
                        frame,
                        outline_area,
                        file_layout_area,
                        theme,
                        outline_focused,
                    ),
                }
            } else {
                render_layout_placeholders(
                    frame,
                    outline_area,
                    file_layout_area,
                    theme,
                    outline_focused,
                );
            }
        }
        Some(Err(reason)) => {
            render_layout_placeholders(
                frame,
                outline_area,
                file_layout_area,
                theme,
                outline_focused,
            );
            render_message(
                frame,
                file_list_area,
                theme,
                vec![
                    ratatui::text::Line::styled(
                        "Unable to load data files",
                        theme.style("keyword"),
                    ),
                    ratatui::text::Line::from(format!("Version: {version}")),
                    ratatui::text::Line::from(""),
                    ratatui::text::Line::from(reason.to_string()),
                ],
            );
        }
        None => {
            render_layout_placeholders(
                frame,
                outline_area,
                file_layout_area,
                theme,
                outline_focused,
            );
            render_message(
                frame,
                file_list_area,
                theme,
                vec![ratatui::text::Line::from("Data file metadata not loaded.")],
            );
        }
    }
}

fn render_layout_placeholders(
    frame: &mut Frame,
    outline_area: Rect,
    file_layout_area: Rect,
    theme: &Theme,
    outline_focused: bool,
) {
    render_placeholder(
        frame,
        outline_area,
        theme,
        INNER_TITLES[0],
        outline_focused,
        vec![ratatui::text::Line::from("No layout outline loaded.")],
    );
    render_placeholder(
        frame,
        file_layout_area,
        theme,
        INNER_TITLES[1],
        false,
        vec![ratatui::text::Line::from("No file layout loaded.")],
    );
}

fn render_file_layouts(
    frame: &mut Frame,
    outline_area: Rect,
    file_layout_area: Rect,
    theme: &Theme,
    layout: &FileLayoutInfo,
    selection: SelectionState<'_>,
) {
    let expanded = selection
        .layout_outline_key
        .and_then(|key| selection.layout_outline_expanded.get(&key));
    let selected_outline_key = render_layout_outline(
        frame,
        outline_area,
        theme,
        layout,
        expanded,
        selection.layout_outline_selected,
        selection.focus == Some(DataFilesFocus::LayoutOutline),
    );
    render_file_layout_map(
        frame,
        file_layout_area,
        theme,
        layout,
        selected_outline_key.as_deref(),
    );
}

pub(crate) fn render_layout_outline(
    frame: &mut Frame,
    area: Rect,
    theme: &Theme,
    layout: &FileLayoutInfo,
    expanded: Option<&BTreeSet<String>>,
    selected: Option<usize>,
    focused: bool,
) -> Option<String> {
    let tree = layout_outline_tree(layout, expanded);
    let visible_nodes = tree.visible_nodes();
    let selected_line =
        selected.and_then(|selected| visible_nodes.iter().position(|id| *id == selected));
    let lines = visible_nodes
        .iter()
        .copied()
        .filter_map(|id| {
            let node = tree.node(id)?;
            let marker = if node.children.is_empty() {
                "•"
            } else if node.expanded {
                "▾"
            } else {
                "▸"
            };
            let text = format!("{}{} {}", "  ".repeat(node.depth), marker, node.label);
            let line = if selected == Some(id) {
                ratatui::text::Line::styled(
                    text,
                    super::pane_style(theme)
                        .bg(theme.color("bg.active").into())
                        .fg(theme.color("accent.primary").into())
                        .add_modifier(Modifier::BOLD),
                )
            } else {
                ratatui::text::Line::from(text)
            };
            Some(line)
        })
        .collect::<Vec<_>>();
    let outline_height = usize::from(tab_content_block(theme).inner(area).height);
    let outline_scroll = layout_outline_scroll_offset(selected_line, lines.len(), outline_height);
    let lines = lines
        .into_iter()
        .skip(outline_scroll)
        .take(outline_height)
        .collect();
    render_layout_pane(frame, area, theme, INNER_TITLES[0], focused, lines);
    selected
        .and_then(|id| tree.node(id))
        .map(|node| node.key.clone())
}

fn layout_outline_scroll_offset(
    selected_line: Option<usize>,
    line_count: usize,
    viewport_height: usize,
) -> usize {
    let Some(selected_line) = selected_line else {
        return 0;
    };
    if viewport_height == 0 {
        return 0;
    }
    selected_line
        .saturating_add(1)
        .saturating_sub(viewport_height)
        .min(line_count.saturating_sub(viewport_height))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FileLayoutTileKind {
    Data,
    Page,
    GlobalBuffer,
    Metadata,
    OffsetTable,
    Statistics,
    Dictionary,
    Footer,
}

impl FileLayoutTileKind {
    const ALL: [Self; 8] = [
        Self::Data,
        Self::Page,
        Self::GlobalBuffer,
        Self::Metadata,
        Self::OffsetTable,
        Self::Statistics,
        Self::Dictionary,
        Self::Footer,
    ];

    const fn priority(self) -> u8 {
        match self {
            Self::Data => 0,
            Self::Page => 1,
            Self::GlobalBuffer => 2,
            Self::Metadata => 3,
            Self::OffsetTable => 4,
            Self::Statistics => 5,
            Self::Dictionary => 6,
            Self::Footer => 7,
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::Data => "Data",
            Self::Page => "Pages",
            Self::GlobalBuffer => "Global",
            Self::Metadata => "Metadata",
            Self::OffsetTable => "Tables",
            Self::Statistics => "Statistics",
            Self::Dictionary => "Dictionary",
            Self::Footer => "Footer",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FileLayoutTileRegion {
    key: String,
    kind: FileLayoutTileKind,
    range: ByteRange,
}

pub(crate) fn render_file_layout_map(
    frame: &mut Frame,
    area: Rect,
    theme: &Theme,
    layout: &FileLayoutInfo,
    selected_outline_key: Option<&str>,
) {
    frame.render_widget(Clear, area);
    let block = tab_content_block(theme)
        .title_style(tab_title_style(theme))
        .border_style(theme.style("unfocused_border"))
        .title(INNER_TITLES[1]);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.width == 0 || inner.height < 3 || layout.file_size_bytes == 0 {
        frame.render_widget(
            Paragraph::new("File is too small to render a layout map.")
                .style(super::pane_style(theme)),
            inner,
        );
        return;
    }

    let [map_area, legend_area] =
        Layout::vertical([Constraint::Min(1), Constraint::Length(2)]).areas(inner);
    let [offset_area, tile_area] =
        Layout::horizontal([Constraint::Length(10), Constraint::Min(0)]).areas(map_area);
    let tile_columns = tile_area.width / 2;
    let tile_rows = tile_area.height;
    let tile_count = usize::from(tile_columns) * usize::from(tile_rows);
    if tile_count == 0 {
        frame.render_widget(
            Paragraph::new("File Layout needs a wider pane.").style(super::pane_style(theme)),
            inner,
        );
        return;
    }

    let regions = file_layout_tile_regions(layout);
    let selected_outline_key = selected_outline_key.filter(|key| {
        is_specific_outline_region(key)
            && regions
                .iter()
                .any(|region| region_matches_outline_key(region, key))
    });
    let selected_regions = selected_outline_key
        .map(|key| {
            regions
                .iter()
                .filter(|region| region_matches_outline_key(region, key))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let tile_kinds = tile_kinds_for_layout(layout.file_size_bytes, tile_count, &regions);
    let mut offset_lines = Vec::with_capacity(usize::from(tile_rows));
    for row in 0..tile_rows {
        let index = usize::from(row) * usize::from(tile_columns);
        let range = tile_byte_range(layout.file_size_bytes, index, tile_count);
        offset_lines.push(Line::styled(
            format!("{:08x}", range.offset),
            theme.style("muted"),
        ));
    }
    frame.render_widget(
        Paragraph::new(offset_lines)
            .style(super::pane_style(theme))
            .alignment(ratatui::layout::Alignment::Right),
        offset_area,
    );

    let buffer = frame.buffer_mut();
    for row in 0..tile_rows {
        for column in 0..tile_columns {
            let index = usize::from(row) * usize::from(tile_columns) + usize::from(column);
            let range = tile_byte_range(layout.file_size_bytes, index, tile_count);
            let selected_kind = selected_tile_kind(range, &selected_regions);
            let Some(kind) = selected_kind.or(tile_kinds[index]) else {
                continue;
            };
            let highlighted = selected_kind.is_some();
            let x = tile_area.x + column * 2;
            let y = tile_area.y + row;
            if highlighted {
                let style = Style::default().bg(tile_color(theme, kind, true));
                buffer[(x, y)].set_char(' ').set_style(style);
                buffer[(x + 1, y)].set_char(' ').set_style(style);
            } else {
                let style = Style::default()
                    .fg(tile_color(theme, kind, true))
                    .bg(theme.color("bg.panel").into());
                buffer[(x, y)].set_char('.').set_style(style);
                buffer[(x + 1, y)].set_char(' ').set_style(style);
            }
        }
    }

    render_file_layout_legend(
        frame,
        legend_area,
        theme,
        layout.file_size_bytes,
        &regions,
        selected_outline_key,
    );
}

fn render_file_layout_legend(
    frame: &mut Frame,
    area: Rect,
    theme: &Theme,
    file_size: u64,
    regions: &[FileLayoutTileRegion],
    selected_outline_key: Option<&str>,
) {
    let mut kinds = Vec::new();
    for region in regions {
        if !kinds.contains(&region.kind) {
            kinds.push(region.kind);
        }
    }
    let mut legend = Vec::new();
    for kind in kinds {
        legend.push(Span::styled(
            "  ",
            Style::default().bg(tile_color(theme, kind, true)),
        ));
        legend.push(Span::raw(format!(" {} ", kind.label())));
    }
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(legend),
            Line::styled(
                format!(
                    "0x0 — {:#x}  •  {} physical regions{}",
                    file_size,
                    regions.len(),
                    selected_outline_key.map_or_else(String::new, |key| format!("  •  {key}"))
                ),
                theme.style("muted"),
            ),
        ])
        .style(super::pane_style(theme)),
        area,
    );
}

fn file_layout_tile_regions(layout: &FileLayoutInfo) -> Vec<FileLayoutTileRegion> {
    let mut regions = match &layout.kind {
        FileLayoutKind::V2(v2) => v2_file_layout_tile_regions(layout.file_size_bytes, v2),
        FileLayoutKind::Legacy(legacy) => {
            legacy_file_layout_tile_regions(layout.file_size_bytes, legacy)
        }
    };
    regions.retain(|region| region.range.size > 0 && region.range.offset < layout.file_size_bytes);
    regions
}

fn v2_file_layout_tile_regions(file_size: u64, v2: &V2FileLayout) -> Vec<FileLayoutTileRegion> {
    let mut regions = vec![
        FileLayoutTileRegion {
            key: "physical/0".to_string(),
            kind: FileLayoutTileKind::Data,
            range: ByteRange {
                offset: 0,
                size: v2.footer.column_metadata_start,
            },
        },
        FileLayoutTileRegion {
            key: "physical/1".to_string(),
            kind: FileLayoutTileKind::Metadata,
            range: ByteRange {
                offset: v2.footer.column_metadata_start,
                size: v2
                    .footer
                    .cmo_start
                    .saturating_sub(v2.footer.column_metadata_start),
            },
        },
        FileLayoutTileRegion {
            key: "physical/2".to_string(),
            kind: FileLayoutTileKind::OffsetTable,
            range: ByteRange {
                offset: v2.footer.cmo_start,
                size: v2.footer.gbo_start.saturating_sub(v2.footer.cmo_start),
            },
        },
        FileLayoutTileRegion {
            key: "physical/3".to_string(),
            kind: FileLayoutTileKind::OffsetTable,
            range: ByteRange {
                offset: v2.footer.gbo_start,
                size: v2.footer.footer_start.saturating_sub(v2.footer.gbo_start),
            },
        },
        FileLayoutTileRegion {
            key: "physical/4".to_string(),
            kind: FileLayoutTileKind::Footer,
            range: ByteRange {
                offset: v2.footer.footer_start,
                size: file_size.saturating_sub(v2.footer.footer_start),
            },
        },
    ];
    for (column_index, column) in v2.columns.iter().enumerate() {
        regions.push(FileLayoutTileRegion {
            key: format!("columns/{column_index}/metadata"),
            kind: FileLayoutTileKind::Metadata,
            range: column.metadata,
        });
        regions.extend(
            column
                .buffers
                .iter()
                .enumerate()
                .map(|(buffer_index, &range)| FileLayoutTileRegion {
                    key: format!("columns/{column_index}/metadata-buffer/{buffer_index}"),
                    kind: FileLayoutTileKind::Metadata,
                    range,
                }),
        );
        for (page_index, page) in column.pages.iter().enumerate() {
            regions.extend(
                page.buffers
                    .iter()
                    .enumerate()
                    .map(|(buffer_index, &range)| FileLayoutTileRegion {
                        key: format!(
                            "columns/{column_index}/pages/{page_index}/buffer/{buffer_index}"
                        ),
                        kind: FileLayoutTileKind::Page,
                        range,
                    }),
            );
        }
    }
    regions.extend(
        v2.global_buffers
            .iter()
            .enumerate()
            .map(|(buffer_index, &range)| FileLayoutTileRegion {
                key: format!("global/{buffer_index}"),
                kind: FileLayoutTileKind::GlobalBuffer,
                range,
            }),
    );
    regions
}

fn legacy_file_layout_tile_regions(
    file_size: u64,
    legacy: &LegacyFileLayout,
) -> Vec<FileLayoutTileRegion> {
    let mut regions = vec![
        FileLayoutTileRegion {
            key: "legacy/data".to_string(),
            kind: FileLayoutTileKind::Data,
            range: ByteRange {
                offset: 0,
                size: legacy.footer_start,
            },
        },
        FileLayoutTileRegion {
            key: "legacy/descriptor/metadata".to_string(),
            kind: FileLayoutTileKind::Metadata,
            range: ByteRange {
                offset: legacy.metadata_offset,
                size: legacy.footer_start.saturating_sub(legacy.metadata_offset),
            },
        },
        FileLayoutTileRegion {
            key: "legacy/page-table".to_string(),
            kind: FileLayoutTileKind::OffsetTable,
            range: ByteRange {
                offset: legacy.page_table_position,
                size: legacy.page_table_size,
            },
        },
        FileLayoutTileRegion {
            key: "legacy/footer".to_string(),
            kind: FileLayoutTileKind::Footer,
            range: ByteRange {
                offset: legacy.footer_start,
                size: file_size.saturating_sub(legacy.footer_start),
            },
        },
    ];
    if legacy.descriptor_size > 0 {
        regions.push(FileLayoutTileRegion {
            key: "legacy/descriptor/file".to_string(),
            kind: FileLayoutTileKind::Metadata,
            range: ByteRange {
                offset: 0,
                size: legacy.descriptor_size,
            },
        });
    }
    if let (Some(offset), Some(size)) = (
        legacy.statistics_page_table_position,
        legacy.statistics_page_table_size,
    ) {
        regions.push(FileLayoutTileRegion {
            key: "legacy/statistics-table".to_string(),
            kind: FileLayoutTileKind::OffsetTable,
            range: ByteRange { offset, size },
        });
    }
    regions.extend(
        legacy
            .pages
            .iter()
            .enumerate()
            .map(|(index, page)| FileLayoutTileRegion {
                key: format!("legacy/pages/{index}"),
                kind: FileLayoutTileKind::Page,
                range: page.range,
            }),
    );
    regions.extend(
        legacy
            .statistics_pages
            .iter()
            .enumerate()
            .map(|(index, page)| FileLayoutTileRegion {
                key: format!("legacy/statistics/{index}"),
                kind: FileLayoutTileKind::Statistics,
                range: page.range,
            }),
    );
    regions.extend(
        legacy
            .dictionary_ranges
            .iter()
            .enumerate()
            .map(|(index, &range)| FileLayoutTileRegion {
                key: format!("legacy/dictionaries/{index}"),
                kind: FileLayoutTileKind::Dictionary,
                range,
            }),
    );
    regions
}

fn tile_byte_range(file_size: u64, index: usize, tile_count: usize) -> ByteRange {
    let file_size = u128::from(file_size);
    let index = index as u128;
    let tile_count = tile_count as u128;
    let offset = (file_size * index / tile_count) as u64;
    let end = (file_size * (index + 1) / tile_count) as u64;
    ByteRange {
        offset,
        size: end.saturating_sub(offset),
    }
}

fn tile_kind_for_range(
    tile: ByteRange,
    regions: &[FileLayoutTileRegion],
) -> Option<FileLayoutTileKind> {
    regions
        .iter()
        .filter(|region| ranges_overlap(tile, region.range))
        .max_by_key(|region| (region.kind.priority(), overlap_size(tile, region.range)))
        .map(|region| region.kind)
}

fn tile_kinds_for_layout(
    file_size: u64,
    tile_count: usize,
    regions: &[FileLayoutTileRegion],
) -> Vec<Option<FileLayoutTileKind>> {
    let mut tile_kinds = (0..tile_count)
        .map(|index| tile_kind_for_range(tile_byte_range(file_size, index, tile_count), regions))
        .collect::<Vec<_>>();
    let mut reserved = vec![false; tile_count];
    for kind in FileLayoutTileKind::ALL {
        let Some(region) = regions.iter().find(|region| region.kind == kind) else {
            continue;
        };
        if let Some(index) = tile_kinds
            .iter()
            .position(|tile_kind| *tile_kind == Some(kind))
        {
            reserved[index] = true;
            continue;
        }
        let target = tile_index_for_offset(file_size, tile_count, region.range.offset);
        let replacement = nearest_unreserved_tile(&reserved, target);
        tile_kinds[replacement] = Some(kind);
        reserved[replacement] = true;
    }
    tile_kinds
}

fn tile_index_for_offset(file_size: u64, tile_count: usize, offset: u64) -> usize {
    if file_size == 0 || tile_count == 0 {
        return 0;
    }
    let index = u128::from(offset.min(file_size.saturating_sub(1))) * tile_count as u128
        / u128::from(file_size);
    (index as usize).min(tile_count - 1)
}

fn nearest_unreserved_tile(reserved: &[bool], target: usize) -> usize {
    for distance in 0..reserved.len() {
        let before = target.saturating_sub(distance);
        if !reserved[before] {
            return before;
        }
        let after = target.saturating_add(distance);
        if after < reserved.len() && !reserved[after] {
            return after;
        }
    }
    target
}

fn ranges_overlap(left: ByteRange, right: ByteRange) -> bool {
    left.offset < right.end() && right.offset < left.end()
}

fn overlap_size(left: ByteRange, right: ByteRange) -> u64 {
    left.end()
        .min(right.end())
        .saturating_sub(left.offset.max(right.offset))
}

fn selected_tile_kind(
    tile: ByteRange,
    selected_regions: &[&FileLayoutTileRegion],
) -> Option<FileLayoutTileKind> {
    selected_regions
        .iter()
        .filter(|region| ranges_overlap(tile, region.range))
        .max_by_key(|region| (region.kind.priority(), overlap_size(tile, region.range)))
        .map(|region| region.kind)
}

fn region_matches_outline_key(region: &FileLayoutTileRegion, outline_key: &str) -> bool {
    region.key == outline_key
        || region
            .key
            .strip_prefix(outline_key)
            .is_some_and(|suffix| suffix.starts_with('/'))
}

fn is_specific_outline_region(key: &str) -> bool {
    !matches!(
        key,
        "root"
            | "physical"
            | "columns"
            | "global"
            | "legacy/descriptor"
            | "legacy/pages"
            | "legacy/statistics"
            | "legacy/dictionaries"
    )
}

fn tile_color(theme: &Theme, kind: FileLayoutTileKind, highlighted: bool) -> Color {
    let light = theme.meta.variant == ThemeVariant::Light;
    let (red, green, blue) = match (light, kind) {
        (true, FileLayoutTileKind::Data) => (147, 197, 253),
        (true, FileLayoutTileKind::Page) => (110, 231, 183),
        (true, FileLayoutTileKind::GlobalBuffer) => (196, 181, 253),
        (true, FileLayoutTileKind::Metadata) => (253, 230, 138),
        (true, FileLayoutTileKind::OffsetTable) => (251, 191, 36),
        (true, FileLayoutTileKind::Statistics) => (249, 168, 212),
        (true, FileLayoutTileKind::Dictionary) => (125, 211, 252),
        (true, FileLayoutTileKind::Footer) => (252, 165, 165),
        (false, FileLayoutTileKind::Data) => (30, 64, 175),
        (false, FileLayoutTileKind::Page) => (4, 120, 87),
        (false, FileLayoutTileKind::GlobalBuffer) => (91, 33, 182),
        (false, FileLayoutTileKind::Metadata) => (161, 98, 7),
        (false, FileLayoutTileKind::OffsetTable) => (180, 83, 9),
        (false, FileLayoutTileKind::Statistics) => (157, 23, 77),
        (false, FileLayoutTileKind::Dictionary) => (3, 105, 161),
        (false, FileLayoutTileKind::Footer) => (153, 27, 27),
    };
    if highlighted {
        Color::Rgb(red, green, blue)
    } else {
        let panel = theme.color("bg.panel");
        let (panel_red, panel_green, panel_blue) = panel.to_rgb_tuple();
        Color::Rgb(
            ((u16::from(red) + u16::from(panel_red) * 5) / 6) as u8,
            ((u16::from(green) + u16::from(panel_green) * 5) / 6) as u8,
            ((u16::from(blue) + u16::from(panel_blue) * 5) / 6) as u8,
        )
    }
}

#[allow(dead_code)]
fn layout_outline_lines(layout: &FileLayoutInfo) -> Vec<ratatui::text::Line<'static>> {
    let mut lines = vec![ratatui::text::Line::from("Binary layout")];
    match &layout.kind {
        FileLayoutKind::V2(v2) => append_v2_layout_lines(&mut lines, layout.file_size_bytes, v2),
        FileLayoutKind::Legacy(legacy) => {
            append_legacy_layout_lines(&mut lines, layout.file_size_bytes, legacy)
        }
    }
    lines
}

#[allow(dead_code)]
fn append_v2_layout_lines(
    lines: &mut Vec<ratatui::text::Line<'static>>,
    file_size: u64,
    v2: &V2FileLayout,
) {
    lines.push(ratatui::text::Line::from("├─ Physical regions"));
    let regions = [
        (
            "data + global buffers",
            ByteRange {
                offset: 0,
                size: v2.footer.column_metadata_start,
            },
        ),
        (
            "column metadata",
            ByteRange {
                offset: v2.footer.column_metadata_start,
                size: v2
                    .footer
                    .cmo_start
                    .saturating_sub(v2.footer.column_metadata_start),
            },
        ),
        (
            "column metadata offset table",
            ByteRange {
                offset: v2.footer.cmo_start,
                size: v2.footer.gbo_start.saturating_sub(v2.footer.cmo_start),
            },
        ),
        (
            "global buffer offset table",
            ByteRange {
                offset: v2.footer.gbo_start,
                size: v2.footer.footer_start.saturating_sub(v2.footer.gbo_start),
            },
        ),
        (
            "footer",
            ByteRange {
                offset: v2.footer.footer_start,
                size: file_size.saturating_sub(v2.footer.footer_start),
            },
        ),
    ];
    append_ranges(lines, "│  ", &regions);

    lines.push(ratatui::text::Line::from("├─ Columns"));
    for (column_index, column) in v2.columns.iter().enumerate() {
        let column_last = column_index + 1 == v2.columns.len();
        let column_prefix = if column_last { "└─" } else { "├─" };
        let column_indent = if column_last {
            "│     "
        } else {
            "│  │  "
        };
        lines.push(ratatui::text::Line::from(format!(
            "│  {column_prefix} Column {column_index}"
        )));
        lines.push(ratatui::text::Line::from(format!(
            "{column_indent}├─ encoding: {}",
            column.encoding
        )));
        lines.push(ratatui::text::Line::from(format!(
            "{column_indent}├─ {}",
            range_label("metadata", column.metadata)
        )));
        for (buffer_index, buffer) in column.buffers.iter().enumerate() {
            let prefix = if buffer_index + 1 == column.buffers.len() && column.pages.is_empty() {
                "└─"
            } else {
                "├─"
            };
            lines.push(ratatui::text::Line::from(format!(
                "{column_indent}{prefix} {}",
                range_label(format!("metadata buffer {buffer_index}"), *buffer)
            )));
        }
        let pages_prefix = if column.pages.is_empty() {
            "└─"
        } else {
            "├─"
        };
        lines.push(ratatui::text::Line::from(format!(
            "{column_indent}{pages_prefix} Pages ({})",
            column.pages.len()
        )));
        let page_indent = format!(
            "{column_indent}{}",
            if column.pages.is_empty() {
                "   "
            } else {
                "│  "
            }
        );
        for (page_index, page) in column.pages.iter().enumerate() {
            let page_last = page_index + 1 == column.pages.len();
            let page_prefix = if page_last { "└─" } else { "├─" };
            lines.push(ratatui::text::Line::from(format!(
                "{page_indent}{page_prefix} Page {page_index}: rows={} priority={}",
                page.num_rows, page.priority
            )));
            let page_child_indent =
                format!("{page_indent}{}", if page_last { "   " } else { "│  " });
            lines.push(ratatui::text::Line::from(format!(
                "{page_child_indent}├─ encoding: {}",
                page.encoding
            )));
            for (buffer_index, buffer) in page.buffers.iter().enumerate() {
                let prefix = if buffer_index + 1 == page.buffers.len() {
                    "└─"
                } else {
                    "├─"
                };
                lines.push(ratatui::text::Line::from(format!(
                    "{page_child_indent}{prefix} {}",
                    range_label(format!("buffer {buffer_index}"), *buffer)
                )));
            }
        }
    }

    lines.push(ratatui::text::Line::from("└─ Global buffers"));
    for (buffer_index, buffer) in v2.global_buffers.iter().enumerate() {
        let prefix = if buffer_index + 1 == v2.global_buffers.len() {
            "└─"
        } else {
            "├─"
        };
        lines.push(ratatui::text::Line::from(format!(
            "   {prefix} {}",
            range_label(format!("global buffer {buffer_index}"), *buffer)
        )));
    }
}

#[allow(dead_code)]
fn append_legacy_layout_lines(
    lines: &mut Vec<ratatui::text::Line<'static>>,
    file_size: u64,
    legacy: &LegacyFileLayout,
) {
    lines.push(ratatui::text::Line::from("├─ Descriptor and metadata"));
    if legacy.descriptor_size > 0 {
        lines.push(ratatui::text::Line::from(format!(
            "│  ├─ {}",
            range_label(
                "file descriptor",
                ByteRange {
                    offset: 0,
                    size: legacy.descriptor_size,
                },
            )
        )));
    }
    lines.push(ratatui::text::Line::from(format!(
        "│  └─ {}",
        range_label(
            "metadata",
            ByteRange {
                offset: legacy.metadata_offset,
                size: legacy.footer_start.saturating_sub(legacy.metadata_offset),
            },
        )
    )));
    lines.push(ratatui::text::Line::from(format!(
        "├─ Pages ({})",
        legacy.pages.len()
    )));
    for (index, page) in legacy.pages.iter().enumerate() {
        let prefix = if index + 1 == legacy.pages.len() {
            "└─"
        } else {
            "├─"
        };
        lines.push(ratatui::text::Line::from(format!(
            "│  {prefix} {}",
            range_label(
                format!("field {} batch {}", page.field_id, page.batch),
                page.range,
            )
        )));
    }
    lines.push(ratatui::text::Line::from(format!(
        "├─ Statistics ({})",
        legacy.statistics_pages.len()
    )));
    for (index, page) in legacy.statistics_pages.iter().enumerate() {
        let prefix = if index + 1 == legacy.statistics_pages.len() {
            "└─"
        } else {
            "├─"
        };
        lines.push(ratatui::text::Line::from(format!(
            "│  {prefix} {}",
            range_label(format!("field {} statistics", page.field_id), page.range)
        )));
    }
    lines.push(ratatui::text::Line::from(format!(
        "├─ Dictionaries ({})",
        legacy.dictionary_ranges.len()
    )));
    for (index, range) in legacy.dictionary_ranges.iter().enumerate() {
        let prefix = if index + 1 == legacy.dictionary_ranges.len() {
            "└─"
        } else {
            "├─"
        };
        lines.push(ratatui::text::Line::from(format!(
            "│  {prefix} {}",
            range_label(format!("dictionary {index}"), *range)
        )));
    }
    lines.push(ratatui::text::Line::from(format!(
        "└─ {}",
        range_label(
            "footer",
            ByteRange {
                offset: legacy.footer_start,
                size: file_size.saturating_sub(legacy.footer_start),
            },
        )
    )));
    if !legacy.field_encodings.is_empty() {
        lines.push(ratatui::text::Line::from("   └─ Field encodings"));
        for (field_id, encoding) in &legacy.field_encodings {
            lines.push(ratatui::text::Line::from(format!(
                "      └─ field {field_id}: {encoding}"
            )));
        }
    }
}

#[allow(dead_code)]
fn append_ranges(
    lines: &mut Vec<ratatui::text::Line<'static>>,
    indent: &str,
    ranges: &[(&str, ByteRange); 5],
) {
    let visible_count = ranges.iter().filter(|(_, range)| range.size > 0).count();
    let mut visible_index = 0;
    for (label, range) in ranges {
        if range.size == 0 {
            continue;
        }
        let prefix = if visible_index + 1 == visible_count {
            "└─"
        } else {
            "├─"
        };
        lines.push(ratatui::text::Line::from(format!(
            "{indent}{prefix} {}",
            range_label(*label, *range)
        )));
        visible_index += 1;
    }
}

#[allow(dead_code)]
fn range_label(label: impl Into<String>, range: ByteRange) -> String {
    format!(
        "{}  [{:#x}, {:#x})  {}",
        label.into(),
        range.offset,
        range.end(),
        format_size(Some(range.size))
    )
}

fn render_layout_error(
    frame: &mut Frame,
    outline_area: Rect,
    file_layout_area: Rect,
    theme: &Theme,
    reason: &str,
    focused: bool,
) {
    let content = vec![
        ratatui::text::Line::styled("Unable to parse data file", theme.style("keyword")),
        ratatui::text::Line::from(""),
        ratatui::text::Line::from(reason.to_string()),
    ];
    render_layout_pane(
        frame,
        outline_area,
        theme,
        INNER_TITLES[0],
        focused,
        content.clone(),
    );
    render_placeholder(
        frame,
        file_layout_area,
        theme,
        INNER_TITLES[1],
        false,
        vec![ratatui::text::Line::from("No file layout loaded.")],
    );
}

fn render_layout_pane(
    frame: &mut Frame,
    area: Rect,
    theme: &Theme,
    title: &str,
    focused: bool,
    content: Vec<ratatui::text::Line<'static>>,
) {
    render_placeholder(frame, area, theme, title, focused, content);
}

fn render_message(
    frame: &mut Frame,
    area: Rect,
    theme: &Theme,
    content: Vec<ratatui::text::Line<'static>>,
) {
    frame.render_widget(
        Paragraph::new(content).style(super::pane_style(theme)),
        area,
    );
}

fn data_file_row(
    data_file: &DataFileInfo,
    index: usize,
    selected_data_file: Option<usize>,
    theme: &Theme,
) -> Row<'static> {
    let marked = selected_data_file == Some(index);
    let mut row = Row::new([
        Cell::from(if marked { "▸" } else { "" }),
        Cell::from(data_file.path.clone()),
        Cell::from(data_file.format.clone()),
        Cell::from(format_count(data_file.rows)),
        Cell::from(data_file.columns.to_string()),
        Cell::from(format_size(data_file.size_bytes)),
    ]);
    if marked {
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

fn format_size(size_bytes: Option<u64>) -> String {
    size_bytes.map_or_else(|| "unknown".to_string(), format_bytes)
}

fn selected_fragment_index(
    fragments: &[FragmentInfo],
    selected_fragment: Option<usize>,
) -> Option<usize> {
    selected_fragment
        .filter(|index| *index < fragments.len())
        .or_else(|| (!fragments.is_empty()).then_some(0))
}

fn render_error(
    frame: &mut Frame,
    area: Rect,
    theme: &Theme,
    version: u64,
    reason: &str,
    focused: bool,
) {
    render_placeholder(
        frame,
        area,
        theme,
        COLUMN_TITLES[1],
        focused,
        vec![
            ratatui::text::Line::styled("Unable to load fragments", theme.style("keyword")),
            ratatui::text::Line::from(format!("Version: {version}")),
            ratatui::text::Line::from(""),
            ratatui::text::Line::from(reason.to_string()),
        ],
    );
}

#[cfg(test)]
mod tests {
    use super::{
        ByteRange, FileLayoutTileKind, FileLayoutTileRegion, is_specific_outline_region,
        layout_outline_scroll_offset, selected_tile_kind, tile_byte_range, tile_kind_for_range,
        tile_kinds_for_layout,
    };

    #[test]
    fn maps_file_bytes_evenly_across_tiles() {
        assert_eq!(
            tile_byte_range(1_000, 0, 4),
            ByteRange {
                offset: 0,
                size: 250,
            }
        );
        assert_eq!(
            tile_byte_range(1_000, 3, 4),
            ByteRange {
                offset: 750,
                size: 250,
            }
        );
    }

    #[test]
    fn scrolls_the_outline_to_keep_the_selected_node_visible() {
        assert_eq!(layout_outline_scroll_offset(Some(7), 12, 3), 5);
        assert_eq!(layout_outline_scroll_offset(Some(1), 12, 3), 0);
        assert_eq!(layout_outline_scroll_offset(None, 12, 3), 0);
    }

    #[test]
    fn uses_the_most_specific_overlapping_region_color() {
        let regions = [
            FileLayoutTileRegion {
                key: "physical/0".to_string(),
                kind: FileLayoutTileKind::Data,
                range: ByteRange {
                    offset: 0,
                    size: 1_000,
                },
            },
            FileLayoutTileRegion {
                key: "physical/4".to_string(),
                kind: FileLayoutTileKind::Footer,
                range: ByteRange {
                    offset: 900,
                    size: 100,
                },
            },
        ];

        assert_eq!(
            tile_kind_for_range(
                ByteRange {
                    offset: 875,
                    size: 125,
                },
                &regions,
            ),
            Some(FileLayoutTileKind::Footer)
        );
    }

    #[test]
    fn selected_metadata_overrides_a_shared_footer_tile() {
        let metadata = FileLayoutTileRegion {
            key: "physical/1".to_string(),
            kind: FileLayoutTileKind::Metadata,
            range: ByteRange {
                offset: 900,
                size: 1,
            },
        };

        assert_eq!(
            selected_tile_kind(
                ByteRange {
                    offset: 875,
                    size: 125,
                },
                &[&metadata],
            ),
            Some(FileLayoutTileKind::Metadata)
        );
    }

    #[test]
    fn keeps_a_tile_for_each_present_region_kind() {
        let regions = [
            FileLayoutTileRegion {
                key: "columns/0/pages/0/buffer/0".to_string(),
                kind: FileLayoutTileKind::Page,
                range: ByteRange {
                    offset: 0,
                    size: 900,
                },
            },
            FileLayoutTileRegion {
                key: "physical/1".to_string(),
                kind: FileLayoutTileKind::Metadata,
                range: ByteRange {
                    offset: 900,
                    size: 1,
                },
            },
            FileLayoutTileRegion {
                key: "physical/2".to_string(),
                kind: FileLayoutTileKind::OffsetTable,
                range: ByteRange {
                    offset: 901,
                    size: 1,
                },
            },
            FileLayoutTileRegion {
                key: "physical/4".to_string(),
                kind: FileLayoutTileKind::Footer,
                range: ByteRange {
                    offset: 900,
                    size: 100,
                },
            },
        ];
        let tiles = tile_kinds_for_layout(1_000, 8, &regions);

        for kind in [
            FileLayoutTileKind::Page,
            FileLayoutTileKind::Metadata,
            FileLayoutTileKind::OffsetTable,
            FileLayoutTileKind::Footer,
        ] {
            assert!(tiles.contains(&Some(kind)), "missing {kind:?}");
        }
    }

    #[test]
    fn highlights_ranges_for_a_selected_concrete_outline_region() {
        let regions = [FileLayoutTileRegion {
            key: "columns/3/pages/1/buffer/0".to_string(),
            kind: FileLayoutTileKind::Page,
            range: ByteRange {
                offset: 400,
                size: 100,
            },
        }];

        assert_eq!(
            selected_tile_kind(
                ByteRange {
                    offset: 450,
                    size: 20,
                },
                &[&regions[0]],
            ),
            Some(FileLayoutTileKind::Page)
        );
        assert!(!is_specific_outline_region("root"));
        assert!(!is_specific_outline_region("columns"));
        assert!(is_specific_outline_region("columns/3"));
        assert!(is_specific_outline_region("physical/1"));
    }
}
