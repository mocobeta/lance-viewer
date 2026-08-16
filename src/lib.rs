use std::{
    error::Error,
    sync::mpsc::{self, TryRecvError},
    thread,
    time::Duration,
};

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::{
    DefaultTerminal,
    layout::{Constraint, Layout, Margin, Spacing},
    style::{Modifier, Style},
    text::Line,
    widgets::{Block, BorderType, Paragraph, Tabs},
};

mod app;
mod cli;
mod dataset;
mod formatting;
mod tabs;

pub use app::AppState;
pub use cli::{Cli, StartupTheme};

fn load_theme(light_mode: bool) -> opaline::Theme {
    opaline::load_by_name(if light_mode { "one-light" } else { "one-dark" })
        .expect("bundled One Light and One Dark themes must be available")
}

pub fn run(
    terminal: &mut DefaultTerminal,
    uri: Option<String>,
    startup_theme: Option<StartupTheme>,
) -> Result<(), Box<dyn Error>> {
    let uri = uri.unwrap_or_else(|| ".".to_string());
    let (dataset_sender, dataset_receiver) = mpsc::sync_channel(1);
    let dataset_uri = uri.clone();
    thread::spawn(move || {
        let _ = dataset_sender.send(load_dataset_info(&dataset_uri));
    });

    let mut state = AppState::with_dataset_state(Some(uri), app::DatasetState::Loading);
    if let Some(startup_theme) = startup_theme {
        state.light_mode = startup_theme.light_mode();
    }
    load_selected_data_file(&mut state);
    let mut theme = load_theme(state.light_mode);

    loop {
        match dataset_receiver.try_recv() {
            Ok(dataset_state) => {
                state.dataset_state = dataset_state;
                state.storage_expanded = match &state.dataset_state {
                    app::DatasetState::Loaded(dataset_info) => {
                        dataset_info.storage_layout.default_expanded()
                    }
                    _ => std::collections::BTreeSet::from([dataset::storage_root_key(".")]),
                };
                load_selected_data_file(&mut state);
            }
            Err(TryRecvError::Empty) | Err(TryRecvError::Disconnected) => {}
        }

        terminal.draw(|frame| {
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
                state.set_help_max_scroll(tabs::help_max_scroll(layout[2], &theme));
            }

            let uri = state.uri.as_deref().unwrap_or("not set");
            frame.render_widget(
                Paragraph::new(vec![
                    Line::styled("Lance Format Viewer", theme.style("keyword")),
                    Line::styled(format!("URI: {uri}"), theme.style("muted")),
                ])
                .block(
                    tabs::pane_block(&theme)
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
                        &theme,
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
                &theme,
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
        })?;

        if event::poll(Duration::from_millis(250))?
            && let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            let key_code = match key.code {
                KeyCode::Char('h') => KeyCode::Left,
                KeyCode::Char('j') => KeyCode::Down,
                KeyCode::Char('k') => KeyCode::Up,
                KeyCode::Char('l') => KeyCode::Right,
                key_code => key_code,
            };

            match key_code {
                KeyCode::Char('q') => return Ok(()),
                KeyCode::Esc if state.selected_tab == 1 && state.tables_selected.is_some() => {
                    state.reset_tables_widgets();
                }
                KeyCode::Esc
                    if state.selected_tab == 2
                        && state.data_files_focus == Some(app::DataFilesFocus::LayoutOutline) =>
                {
                    state.focus_data_files();
                }
                KeyCode::Esc if state.selected_tab == 2 && state.data_files_focus.is_some() => {
                    state.reset_data_files_widgets();
                }
                KeyCode::Esc
                    if state.selected_tab == 3
                        && state.indices_focus == Some(app::IndicesFocus::LayoutOutline) =>
                {
                    state.focus_index_details();
                }
                KeyCode::Esc if state.selected_tab == 3 && state.indices_focus.is_some() => {
                    state.reset_indices_widgets();
                }
                KeyCode::Esc if state.selected_tab == tabs::STORAGE_LAYOUT_TAB => {
                    state.reset_storage_widgets();
                }
                KeyCode::Enter
                    if state.selected_tab == 2
                        && state.data_files_focus == Some(app::DataFilesFocus::DataFiles) =>
                {
                    state.focus_data_files();
                    load_selected_data_file(&mut state);
                    state.focus_layout_outline();
                }
                KeyCode::Enter
                    if state.selected_tab == 1
                        && state.tables_focus == app::TablesFocus::Details
                        && state.tables_detail_selected == 2 =>
                {
                    state.selected_tab = tabs::DATA_FILES_TAB;
                    state.focus_data_files_fragments();
                    load_selected_data_file(&mut state);
                }
                KeyCode::Enter
                    if state.selected_tab == 1
                        && state.tables_focus == app::TablesFocus::Details
                        && state.tables_detail_selected == 3 =>
                {
                    open_selected_table_index(&mut state, load_selected_indices);
                }
                KeyCode::Enter
                    if state.selected_tab == 2
                        && state.data_files_focus == Some(app::DataFilesFocus::LayoutOutline) =>
                {
                    state.toggle_layout_outline();
                }
                KeyCode::Enter
                    if state.selected_tab == 3
                        && state.indices_focus == Some(app::IndicesFocus::LayoutOutline) =>
                {
                    state.toggle_index_layout_outline();
                }
                KeyCode::Enter
                    if state.selected_tab == 3
                        && state.indices_focus == Some(app::IndicesFocus::Details)
                        && state.indices_detail_selected == 1 =>
                {
                    state.focus_index_layout_outline();
                }
                KeyCode::Enter if state.selected_tab == tabs::STORAGE_LAYOUT_TAB => {
                    state.storage_focus = true;
                    state.toggle_storage_selection();
                }
                KeyCode::Up
                    if state.selected_tab == 2
                        && state.data_files_focus == Some(app::DataFilesFocus::LayoutOutline) =>
                {
                    if state.layout_outline_selected.is_none() {
                        state.focus_data_files();
                    } else {
                        state.move_layout_outline_selection(-1);
                        if state.layout_outline_selected.is_none() {
                            state.focus_data_files();
                        }
                    }
                }
                KeyCode::Down
                    if state.selected_tab == 2
                        && state.data_files_focus == Some(app::DataFilesFocus::LayoutOutline) =>
                {
                    state.move_layout_outline_selection(1);
                }
                KeyCode::Up
                    if state.selected_tab == 3
                        && state.indices_focus == Some(app::IndicesFocus::LayoutOutline) =>
                {
                    if state.index_layout_outline_selected.is_none() {
                        state.focus_index_details();
                    } else {
                        state.move_index_layout_outline_selection(-1);
                        if state.index_layout_outline_selected.is_none() {
                            state.focus_index_details();
                        }
                    }
                }
                KeyCode::Down
                    if state.selected_tab == 3
                        && state.indices_focus == Some(app::IndicesFocus::LayoutOutline) =>
                {
                    state.move_index_layout_outline_selection(1);
                }
                KeyCode::Up
                    if state.selected_tab == 2
                        && state.data_files_focus == Some(app::DataFilesFocus::DataFiles) =>
                {
                    if state
                        .data_file_selected
                        .is_none_or(|selected| selected == 0)
                    {
                        state.reset_data_files_widgets();
                    } else {
                        state.move_data_file_selection(-1);
                        load_selected_data_file(&mut state);
                    }
                }
                KeyCode::Down
                    if state.selected_tab == 2
                        && state.data_files_focus == Some(app::DataFilesFocus::DataFiles) =>
                {
                    state.move_data_file_selection(1);
                    load_selected_data_file(&mut state);
                }
                KeyCode::Up
                    if state.selected_tab == 1
                        && state.tables_focus == app::TablesFocus::Details
                        && state.tables_detail_selected == 0 =>
                {
                    state.reset_tables_widgets();
                }
                KeyCode::Up
                    if state.selected_tab == 1
                        && state.tables_focus == app::TablesFocus::Details
                        && state.tables_detail_selected == 1 =>
                {
                    if state.schema_selected.is_none_or(|selected| selected == 0) {
                        state.reset_tables_widgets();
                    } else {
                        state.move_schema_selection(-1);
                    }
                }
                KeyCode::Down
                    if state.selected_tab == 1
                        && state.tables_focus == app::TablesFocus::Details
                        && state.tables_detail_selected == 1 =>
                {
                    state.move_schema_selection(1);
                }
                KeyCode::Up
                    if state.selected_tab == 1
                        && state.tables_focus == app::TablesFocus::Details
                        && state.tables_detail_selected == 3 =>
                {
                    if state
                        .tables_index_selected
                        .is_none_or(|selected| selected == 0)
                    {
                        state.reset_tables_widgets();
                    } else {
                        state.move_tables_index_selection(-1);
                    }
                }
                KeyCode::Down
                    if state.selected_tab == 1
                        && state.tables_focus == app::TablesFocus::Details
                        && state.tables_detail_selected == 3 =>
                {
                    state.move_tables_index_selection(1);
                }
                KeyCode::Up
                    if state.selected_tab == 1
                        && state.tables_focus == app::TablesFocus::Details
                        && state.tables_detail_selected == 2 =>
                {
                    if state.fragment_selected.is_none_or(|selected| selected == 0) {
                        state.reset_tables_widgets();
                    } else {
                        state.move_fragment_selection(-1);
                    }
                }
                KeyCode::Down
                    if state.selected_tab == 1
                        && state.tables_focus == app::TablesFocus::Details
                        && state.tables_detail_selected == 2 =>
                {
                    state.move_fragment_selection(1);
                }
                KeyCode::Up
                    if state.selected_tab == 2
                        && state.data_files_focus == Some(app::DataFilesFocus::Fragments) =>
                {
                    if state.fragment_selected.is_none_or(|selected| selected == 0) {
                        state.reset_data_files_widgets();
                    } else {
                        state.move_fragment_selection(-1);
                        load_selected_data_file(&mut state);
                    }
                }
                KeyCode::Down
                    if state.selected_tab == 2
                        && state.data_files_focus == Some(app::DataFilesFocus::Fragments) =>
                {
                    state.move_fragment_selection(1);
                    load_selected_data_file(&mut state);
                }
                KeyCode::Up
                    if state.selected_tab == 3
                        && state.indices_focus == Some(app::IndicesFocus::Indices) =>
                {
                    if state.index_selected.is_none_or(|selected| selected == 0) {
                        state.reset_indices_widgets();
                    } else {
                        state.move_index_selection(-1);
                    }
                }
                KeyCode::Down
                    if state.selected_tab == 3
                        && state.indices_focus == Some(app::IndicesFocus::Indices) =>
                {
                    state.move_index_selection(1);
                }
                KeyCode::Up
                    if state.selected_tab == 3
                        && state.indices_focus == Some(app::IndicesFocus::Details)
                        && state.indices_detail_selected == 0 =>
                {
                    state.reset_indices_widgets();
                }
                KeyCode::Up
                    if state.selected_tab == 3
                        && state.indices_focus == Some(app::IndicesFocus::Details)
                        && state.indices_detail_selected == 1 =>
                {
                    if state
                        .index_file_selected
                        .is_none_or(|selected| selected == 0)
                    {
                        state.reset_indices_widgets();
                    } else {
                        state.move_index_file_selection(-1);
                        load_selected_index_file(&mut state);
                    }
                }
                KeyCode::Down
                    if state.selected_tab == 3
                        && state.indices_focus == Some(app::IndicesFocus::Details)
                        && state.indices_detail_selected == 1 =>
                {
                    state.move_index_file_selection(1);
                    load_selected_index_file(&mut state);
                }
                KeyCode::Up
                    if state.selected_tab == 1
                        && state.tables_focus == app::TablesFocus::Versions =>
                {
                    if let Some(version) = state.move_tables_selection(-1) {
                        load_selected_version(&mut state, version);
                    } else {
                        state.reset_tables_widgets();
                    }
                }
                KeyCode::Up
                    if state.selected_tab == 2
                        && (state.data_files_focus.is_none()
                            || state.data_files_focus == Some(app::DataFilesFocus::Versions)) =>
                {
                    state.data_files_focus = Some(app::DataFilesFocus::Versions);
                    state.focus_versions();
                    if let Some(version) = state.move_tables_selection(-1) {
                        load_selected_version(&mut state, version);
                        load_selected_data_file(&mut state);
                    } else {
                        state.reset_data_files_widgets();
                    }
                }
                KeyCode::Up
                    if state.selected_tab == 3
                        && (state.indices_focus.is_none()
                            || state.indices_focus == Some(app::IndicesFocus::Versions)) =>
                {
                    state.focus_indices_versions();
                    if let Some(version) = state.move_indices_version_selection(-1) {
                        load_selected_indices(&mut state, version);
                    } else {
                        state.reset_indices_widgets();
                    }
                }
                KeyCode::Down
                    if state.selected_tab == 1
                        && state.tables_focus == app::TablesFocus::Versions =>
                {
                    if let Some(version) = state.move_tables_selection(1) {
                        load_selected_version(&mut state, version);
                    }
                }
                KeyCode::Down
                    if state.selected_tab == 2
                        && (state.data_files_focus.is_none()
                            || state.data_files_focus == Some(app::DataFilesFocus::Versions)) =>
                {
                    state.data_files_focus = Some(app::DataFilesFocus::Versions);
                    state.focus_versions();
                    if let Some(version) = state.move_tables_selection(1) {
                        load_selected_version(&mut state, version);
                        load_selected_data_file(&mut state);
                    } else {
                        state.reset_data_files_widgets();
                    }
                }
                KeyCode::Down
                    if state.selected_tab == 3
                        && (state.indices_focus.is_none()
                            || state.indices_focus == Some(app::IndicesFocus::Versions)) =>
                {
                    state.focus_indices_versions();
                    if let Some(version) = state.move_indices_version_selection(1) {
                        load_selected_indices(&mut state, version);
                    } else {
                        state.reset_indices_widgets();
                    }
                }
                KeyCode::Up if state.selected_tab == tabs::STORAGE_LAYOUT_TAB => {
                    if state.storage_selection_is_at_start() {
                        state.reset_storage_widgets();
                    } else {
                        state.storage_focus = true;
                        state.move_storage_selection(-1);
                    }
                }
                KeyCode::Up if state.selected_tab == tabs::HELP_TAB => {
                    state.scroll_help(-1);
                }
                KeyCode::Down if state.selected_tab == tabs::STORAGE_LAYOUT_TAB => {
                    state.storage_focus = true;
                    state.move_storage_selection(1);
                }
                KeyCode::Down if state.selected_tab == tabs::HELP_TAB => {
                    state.scroll_help(1);
                }
                KeyCode::Char('t') => {
                    state.toggle_theme();
                    theme = load_theme(state.light_mode);
                }
                KeyCode::Right
                    if state.selected_tab == 1
                        && state.tables_selected.is_some()
                        && state.tables_focus == app::TablesFocus::Versions =>
                {
                    state.focus_details();
                }
                KeyCode::Right
                    if state.selected_tab == 1
                        && state.tables_focus == app::TablesFocus::Details =>
                {
                    if state.move_tables_detail(1)
                        && state.tables_detail_selected == 3
                        && let Some(version) = state.tables_selected
                    {
                        load_selected_indices(&mut state, version);
                    }
                }
                KeyCode::Right
                    if state.selected_tab == 2
                        && state.data_files_focus == Some(app::DataFilesFocus::Versions) =>
                {
                    state.data_files_focus = Some(app::DataFilesFocus::Fragments);
                }
                KeyCode::Right
                    if state.selected_tab == 2
                        && state.data_files_focus == Some(app::DataFilesFocus::Fragments) =>
                {
                    state.focus_data_files();
                    load_selected_data_file(&mut state);
                }
                KeyCode::Right
                    if state.selected_tab == 3
                        && state.indices_focus == Some(app::IndicesFocus::Versions) =>
                {
                    state.focus_indices();
                }
                KeyCode::Right
                    if state.selected_tab == 3
                        && state.indices_focus == Some(app::IndicesFocus::Indices) =>
                {
                    state.focus_index_details();
                    load_selected_index_file(&mut state);
                }
                KeyCode::Right
                    if state.selected_tab == 3
                        && state.indices_focus == Some(app::IndicesFocus::Details) =>
                {
                    state.move_indices_detail(1);
                    if state.indices_detail_selected == 1 {
                        load_selected_index_file(&mut state);
                    }
                }
                KeyCode::Left
                    if state.selected_tab == 1
                        && state.tables_focus == app::TablesFocus::Details
                        && state.tables_detail_selected > 0 =>
                {
                    state.move_tables_detail(-1);
                }
                KeyCode::Left
                    if state.selected_tab == 1
                        && state.tables_focus == app::TablesFocus::Details =>
                {
                    state.focus_versions();
                }
                KeyCode::Left
                    if state.selected_tab == 2
                        && state.data_files_focus == Some(app::DataFilesFocus::DataFiles) =>
                {
                    state.return_to_fragments();
                }
                KeyCode::Left
                    if state.selected_tab == 2
                        && state.data_files_focus == Some(app::DataFilesFocus::LayoutOutline) =>
                {
                    state.focus_data_files();
                }
                KeyCode::Left
                    if state.selected_tab == 2
                        && state.data_files_focus == Some(app::DataFilesFocus::Fragments) =>
                {
                    state.data_files_focus = Some(app::DataFilesFocus::Versions);
                }
                KeyCode::Left
                    if state.selected_tab == 3
                        && state.indices_focus == Some(app::IndicesFocus::LayoutOutline) =>
                {
                    state.indices_focus = Some(app::IndicesFocus::Details);
                }
                KeyCode::Left
                    if state.selected_tab == 3
                        && state.indices_focus == Some(app::IndicesFocus::Details) =>
                {
                    if !state.move_indices_detail(-1) {
                        state.return_to_indices();
                    }
                }
                KeyCode::Left
                    if state.selected_tab == 3
                        && state.indices_focus == Some(app::IndicesFocus::Indices) =>
                {
                    state.focus_indices_versions();
                }
                KeyCode::Left
                    if (state.selected_tab != 1
                        && state.selected_tab != 2
                        && state.selected_tab != 3)
                        || (state.selected_tab == 1 && state.tables_selected.is_none())
                        || (state.selected_tab == 2 && state.data_files_focus.is_none())
                        || (state.selected_tab == 3 && state.indices_focus.is_none()) =>
                {
                    state.previous_tab(tabs::TITLES.len());
                }
                KeyCode::Right
                    if (state.selected_tab != 1
                        && state.selected_tab != 2
                        && state.selected_tab != 3)
                        || (state.selected_tab == 1 && state.tables_selected.is_none())
                        || (state.selected_tab == 2 && state.data_files_focus.is_none())
                        || (state.selected_tab == 3 && state.indices_focus.is_none()) =>
                {
                    state.next_tab(tabs::TITLES.len());
                }
                KeyCode::Tab
                    if (state.selected_tab != 1
                        && state.selected_tab != 2
                        && state.selected_tab != 3)
                        || (state.selected_tab == 1 && state.tables_selected.is_none())
                        || (state.selected_tab == 2 && state.data_files_focus.is_none())
                        || (state.selected_tab == 3 && state.indices_focus.is_none()) =>
                {
                    state.next_tab(tabs::TITLES.len());
                }
                _ => {}
            }
        }
    }
}

fn load_dataset_info(uri: &str) -> app::DatasetState {
    match dataset::load_dataset_info(uri) {
        Ok(dataset_info) => app::DatasetState::Loaded(Box::new(dataset_info)),
        Err(error) => app::DatasetState::Unavailable {
            uri: uri.to_string(),
            reason: error.to_string(),
        },
    }
}

fn load_selected_version(state: &mut AppState, version: u64) {
    let Some(uri) = state.uri.clone() else {
        return;
    };
    let needs_load = match &state.dataset_state {
        app::DatasetState::Loaded(dataset_info) => {
            !dataset_info.manifest_cache.contains_key(&version)
                || !dataset_info.schema_cache.contains_key(&version)
        }
        _ => false,
    };
    if !needs_load {
        state.ensure_tables_index_selection();
        return;
    }

    let details = dataset::load_version_details(&uri, version);
    if let app::DatasetState::Loaded(dataset_info) = &mut state.dataset_state {
        match details {
            Ok(details) => {
                dataset_info
                    .manifest_cache
                    .insert(version, Ok(details.manifest));
                dataset_info
                    .schema_cache
                    .insert(version, Ok(details.fields));
            }
            Err(error) => {
                let reason = error.to_string();
                dataset_info
                    .manifest_cache
                    .insert(version, Err(reason.clone()));
                dataset_info.schema_cache.insert(version, Err(reason));
            }
        }
    }
}

fn load_selected_indices(state: &mut AppState, version: u64) {
    let Some(uri) = state.uri.clone() else {
        return;
    };
    load_selected_version(state, version);
    let needs_load = match &state.dataset_state {
        app::DatasetState::Loaded(dataset_info) => !dataset_info.index_cache.contains_key(&version),
        _ => false,
    };
    if !needs_load {
        return;
    }

    let indices = dataset::load_index_details(&uri, version).map_err(|error| error.to_string());
    if let app::DatasetState::Loaded(dataset_info) = &mut state.dataset_state {
        dataset_info.index_cache.insert(version, indices);
    }
    state.ensure_tables_index_selection();
}

fn open_selected_table_index(state: &mut AppState, load_indices: impl FnOnce(&mut AppState, u64)) {
    let Some(version) = state.tables_selected else {
        return;
    };
    load_indices(state, version);
    state.selected_tab = tabs::INDICES_TAB;
    state.focus_selected_table_index();
}

fn load_selected_data_file(state: &mut AppState) {
    let Some(uri) = state.uri.clone() else {
        return;
    };
    let request = match &state.dataset_state {
        app::DatasetState::Loaded(dataset_info) => {
            let version = state
                .tables_selected
                .unwrap_or(dataset_info.current_version);
            let Some(Ok(manifest)) = dataset_info.manifest_cache.get(&version) else {
                return;
            };
            let fragment_index = state.fragment_selected.unwrap_or(0);
            let Some(fragment) = manifest.fragments.get(fragment_index) else {
                return;
            };
            let data_file_index = state.data_file_selected.unwrap_or(0);
            if data_file_index >= fragment.data_file_details.len() {
                return;
            }
            (version, fragment.id, data_file_index)
        }
        _ => return,
    };
    let (version, fragment_id, data_file_index) = request;
    let needs_load =
        match &state.dataset_state {
            app::DatasetState::Loaded(dataset_info) => !dataset_info
                .file_layout_cache
                .contains_key(&(version, fragment_id, data_file_index)),
            _ => false,
        };
    if !needs_load {
        state
            .layout_outline_expanded
            .entry((version, fragment_id, data_file_index))
            .or_insert_with(dataset::layout_outline_default_expanded);
        state.reset_layout_outline_selection();
        return;
    }

    let cache_key = (version, fragment_id, data_file_index);
    let layout = dataset::load_data_file_layout(&uri, version, fragment_id, data_file_index)
        .map_err(|error| error.to_string());
    if let app::DatasetState::Loaded(dataset_info) = &mut state.dataset_state {
        dataset_info.file_layout_cache.insert(cache_key, layout);
    }
    state
        .layout_outline_expanded
        .entry(cache_key)
        .or_insert_with(dataset::layout_outline_default_expanded);
    state.reset_layout_outline_selection();
}

fn load_selected_index_file(state: &mut AppState) {
    let Some(uri) = state.uri.clone() else {
        return;
    };
    let request = match &state.dataset_state {
        app::DatasetState::Loaded(dataset_info) => {
            let version = state
                .indices_selected_version
                .unwrap_or(dataset_info.current_version);
            let Some(index_selected) = state.index_selected else {
                return;
            };
            let Some(file_selected) = state.index_file_selected else {
                return;
            };
            let Some(Ok(indices)) = dataset_info.index_cache.get(&version) else {
                return;
            };
            let Some(index) = indices.get(index_selected) else {
                return;
            };
            let Some(file) = index
                .files
                .as_ref()
                .and_then(|files| files.get(file_selected))
            else {
                return;
            };
            Some((
                version,
                index.uuid.clone(),
                index.base_id,
                file.path.clone(),
            ))
        }
        _ => None,
    };
    let Some((version, uuid, base_id, file_path)) = request else {
        return;
    };
    let cache_key = (version, uuid.clone(), file_path.clone());
    let needs_load = match &state.dataset_state {
        app::DatasetState::Loaded(dataset_info) => !dataset_info
            .index_file_layout_cache
            .contains_key(&cache_key),
        _ => false,
    };
    if !needs_load {
        let layout_loaded = match &state.dataset_state {
            app::DatasetState::Loaded(dataset_info) => dataset_info
                .index_file_layout_cache
                .get(&cache_key)
                .is_some_and(|layout| layout.is_ok()),
            _ => false,
        };
        if layout_loaded {
            state
                .index_layout_outline_expanded
                .entry(cache_key.clone())
                .or_insert_with(dataset::layout_outline_default_expanded);
        }
        state.reset_index_layout_outline_selection();
        return;
    }

    let layout = dataset::load_index_file_layout(&uri, version, &uuid, base_id, &file_path)
        .map_err(|error| error.to_string());
    if let app::DatasetState::Loaded(dataset_info) = &mut state.dataset_state {
        dataset_info
            .index_file_layout_cache
            .insert(cache_key.clone(), layout);
    }
    state
        .index_layout_outline_expanded
        .entry(cache_key)
        .or_insert_with(dataset::layout_outline_default_expanded);
    state.reset_index_layout_outline_selection();
}

#[cfg(test)]
mod tests {
    use super::{AppState, app, open_selected_table_index, tabs};

    #[test]
    fn loads_selected_version_before_opening_table_index_in_indices_tab() {
        let mut state = AppState::new(None);
        state.selected_tab = 1;
        state.tables_selected = Some(2);
        state.tables_focus = app::TablesFocus::Details;
        state.tables_detail_selected = 3;
        state.tables_index_selected = Some(1);
        let mut loaded_version = None;

        open_selected_table_index(&mut state, |state, version| {
            assert_eq!(state.selected_tab, 1);
            loaded_version = Some(version);
        });

        assert_eq!(loaded_version, Some(2));
        assert_eq!(state.selected_tab, tabs::INDICES_TAB);
        assert_eq!(state.indices_selected_version, Some(2));
        assert_eq!(state.indices_focus, Some(app::IndicesFocus::Indices));
    }
}
