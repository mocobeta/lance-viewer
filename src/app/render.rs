use ratatui::{
    Frame,
    layout::{Constraint, Layout, Margin, Spacing},
    style::{Modifier, Style},
    text::Line,
    widgets::{Block, BorderType, Paragraph, Tabs},
};

use crate::tabs;

use super::AppState;

/// Internal helper for render.
pub(crate) fn render(frame: &mut Frame<'_>, state: &mut AppState, theme: &opaline::Theme) {
    frame.render_widget(
        Block::default().style(
            Style::default()
                .bg(theme.color("bg.panel").into())
                .fg(theme.color("text.primary").into()),
        ),
        frame.area(),
    );

    let layout_area = frame.area().inner(Margin::new(0, 0));
    let layout = Layout::vertical([
        Constraint::Length(4),
        Constraint::Length(3),
        Constraint::Min(0),
        Constraint::Length(3),
    ])
    .spacing(Spacing::Space(0))
    .split(layout_area);

    if state.selected_tab == tabs::HELP_TAB {
        state.set_help_max_scroll(tabs::help_max_scroll(layout[2], theme));
    }

    let uri = state.uri.as_deref().unwrap_or("not set");
    frame.render_widget(
        Paragraph::new(vec![
            Line::styled("Lance Format Viewer", theme.style("keyword")),
            Line::styled(format!("URI: {uri}"), theme.style("muted")),
        ])
        .block(
            tabs::pane_block(theme)
                .border_type(BorderType::Double)
                .border_style(theme.style("unfocused_border")),
        ),
        layout[0],
    );
    frame.render_widget(
        Tabs::new(tabs::TITLES.iter().copied())
            .select(state.selected_tab)
            .style(theme.style("text.secondary"))
            .highlight_style(
                Style::default()
                    .fg(theme.color("accent.primary").into())
                    .bg(theme.color("bg.active").into())
                    .add_modifier(Modifier::BOLD),
            )
            .block(tabs::tab_block(
                theme,
                match state.selected_tab {
                    1 => state.tables_selected.is_none(),
                    2 => state.data_files_focus.is_none(),
                    3 => state.indices_focus.is_none(),
                    tabs::STORAGE_LAYOUT_TAB => !state.storage_focus,
                    _ => true,
                },
            )),
        layout[1],
    );

    tabs::render(
        frame,
        layout[2],
        state.selected_tab,
        theme,
        tabs::RenderState {
            dataset_state: &state.dataset_state,
            tables_selected: state.tables_selected,
            tables_focus: state.tables_focus,
            data_files_focus: state.data_files_focus,
            data_file_selected: state.data_file_selected,
            tables_detail_selected: state.tables_detail_selected,
            schema_selected: state.schema_selected,
            fragment_selected: state.fragment_selected,
            tables_index_selected: state.tables_index_selected,
            storage_expanded: &state.storage_expanded,
            storage_selected: &state.storage_selected,
            storage_focus: state.storage_focus,
            layout_outline_expanded: &state.layout_outline_expanded,
            layout_outline_selected: state.layout_outline_selected,
            indices_selected_version: state.indices_selected_version,
            indices_focus: state.indices_focus,
            index_selected: state.index_selected,
            index_file_selected: state.index_file_selected,
            indices_detail_selected: state.indices_detail_selected,
            index_layout_outline_expanded: &state.index_layout_outline_expanded,
            index_layout_outline_selected: state.index_layout_outline_selected,
            help_scroll: state.help_scroll,
        },
    );
    let mode = if state.light_mode { "Light" } else { "Dark" };
    frame.render_widget(
        Paragraph::new(format!(
            "Mode: {mode} (t toggle) • ←/→ switch focus • ↑/↓ navigate • Enter open/expand/collapse • Esc parent • q quit"
        ))
        .style(theme.style("muted")),
        layout[3],
    );
}
