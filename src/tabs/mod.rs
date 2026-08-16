mod data_files;
mod help;
mod indices;
mod overview;
mod storage_layout;
mod tables;

use std::collections::{BTreeMap, BTreeSet};

use crate::app::{DataFilesFocus, DatasetState, IndicesFocus, TablesFocus};
use opaline::Theme;
use ratatui::style::Style;
use ratatui::{
    Frame,
    layout::Rect,
    widgets::{Block, Padding},
};

pub(crate) const DATA_FILES_TAB: usize = 2;
pub(crate) const INDICES_TAB: usize = 3;
pub(crate) const STORAGE_LAYOUT_TAB: usize = 4;
pub(crate) const HELP_TAB: usize = 5;

pub(crate) const TITLES: [&str; 6] = [
    overview::TITLE,
    tables::TITLE,
    data_files::TITLE,
    indices::TITLE,
    storage_layout::TITLE,
    help::TITLE,
];

/// Internal representation of RenderState.
pub(crate) struct RenderState<'a> {
    pub(crate) dataset_state: &'a DatasetState,
    pub(crate) tables_selected: Option<u64>,
    pub(crate) tables_focus: TablesFocus,
    pub(crate) data_files_focus: Option<DataFilesFocus>,
    pub(crate) data_file_selected: Option<usize>,
    pub(crate) tables_detail_selected: usize,
    pub(crate) schema_selected: Option<usize>,
    pub(crate) fragment_selected: Option<usize>,
    pub(crate) tables_index_selected: Option<usize>,
    pub(crate) storage_expanded: &'a BTreeSet<String>,
    pub(crate) storage_selected: &'a str,
    pub(crate) storage_focus: bool,
    pub(crate) layout_outline_expanded: &'a BTreeMap<(u64, u64, usize), BTreeSet<String>>,
    pub(crate) layout_outline_selected: Option<usize>,
    pub(crate) indices_selected_version: Option<u64>,
    pub(crate) indices_focus: Option<IndicesFocus>,
    pub(crate) index_selected: Option<usize>,
    pub(crate) index_file_selected: Option<usize>,
    pub(crate) indices_detail_selected: usize,
    pub(crate) index_layout_outline_expanded: &'a BTreeMap<(u64, String, String), BTreeSet<String>>,
    pub(crate) index_layout_outline_selected: Option<usize>,
    pub(crate) help_scroll: usize,
}

/// Internal helper for help max scroll.
pub(crate) fn help_max_scroll(area: Rect, theme: &Theme) -> usize {
    help::max_scroll(area, theme)
}

/// Internal helper for pane style.
pub(crate) fn pane_style(theme: &Theme) -> Style {
    Style::default()
        .fg(theme.color("text.primary").into())
        .bg(theme.color("bg.panel").into())
}

/// Internal helper for pane block.
pub(crate) fn pane_block(theme: &Theme) -> Block<'static> {
    Block::bordered()
        .style(pane_style(theme))
        .border_style(theme.style("unfocused_border"))
}

/// Internal helper for tab content block.
pub(crate) fn tab_content_block(theme: &Theme) -> Block<'static> {
    pane_block(theme).padding(Padding::proportional(1))
}

/// Internal helper for tab title style.
pub(crate) fn tab_title_style(theme: &Theme) -> Style {
    Style::default().fg(theme.color("accent.secondary").into())
}

/// Internal helper for tab block.
pub(crate) fn tab_block(theme: &Theme, focused: bool) -> Block<'static> {
    pane_block(theme).border_style(if focused {
        theme.style("focused_border")
    } else {
        theme.style("unfocused_border")
    })
}

/// Internal helper for render.
pub(crate) fn render(
    frame: &mut Frame,
    area: Rect,
    selected_tab: usize,
    theme: &Theme,
    render_state: RenderState<'_>,
) {
    match selected_tab {
        0 => overview::render(frame, area, theme, render_state.dataset_state),
        1 => tables::render(
            frame,
            area,
            theme,
            render_state.dataset_state,
            tables::RenderState {
                selected_version: render_state.tables_selected,
                tables_focus: render_state.tables_focus,
                tables_detail_selected: render_state.tables_detail_selected,
                selected_field: render_state.schema_selected,
                selected_fragment: render_state.fragment_selected,
                selected_index: render_state.tables_index_selected,
            },
        ),
        2 => data_files::render(
            frame,
            area,
            theme,
            data_files::RenderState {
                dataset_state: render_state.dataset_state.clone(),
                selected_version: render_state.tables_selected,
                selected_fragment: render_state.fragment_selected,
                selected_data_file: render_state.data_file_selected,
                focus: render_state.data_files_focus,
                layout_outline_expanded: render_state.layout_outline_expanded,
                layout_outline_selected: render_state.layout_outline_selected,
            },
        ),
        3 => indices::render(
            frame,
            area,
            theme,
            render_state.dataset_state,
            indices::RenderState {
                selected_version: render_state.indices_selected_version,
                focus: render_state.indices_focus,
                selected_index: render_state.index_selected,
                selected_file: render_state.index_file_selected,
                selected_detail: render_state.indices_detail_selected,
                layout_outline_expanded: render_state.index_layout_outline_expanded,
                layout_outline_selected: render_state.index_layout_outline_selected,
            },
        ),
        STORAGE_LAYOUT_TAB => storage_layout::render(
            frame,
            area,
            theme,
            render_state.dataset_state,
            render_state.storage_expanded,
            render_state.storage_selected,
            render_state.storage_focus,
        ),
        HELP_TAB => help::render(frame, area, theme, render_state.help_scroll),
        _ => overview::render(frame, area, theme, render_state.dataset_state),
    }
}
