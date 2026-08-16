use std::{
    error::Error,
    sync::mpsc::{self, TryRecvError},
    thread,
};

use crate::{cli::StartupTheme, dataset, tabs};
use crossterm::event::KeyCode;
use ratatui::DefaultTerminal;

use super::input::{self, Action};
use super::loading::{
    load_dataset_info, load_selected_data_file, load_selected_index_file, load_selected_indices,
    load_selected_version, open_selected_table_index,
};
use super::render;
use super::state as app;
use app::AppState;

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

        terminal.draw(|frame| render::render(frame, &mut state, &theme))?;
        if let Some(action) = input::poll_action()? {
            match action {
                Action::Quit => return Ok(()),
                Action::ToggleTheme => {
                    state.toggle_theme();
                    theme = load_theme(state.light_mode);
                }
                Action::Key(key_code) => match key_code {
                    KeyCode::Esc if state.selected_tab == 1 && state.tables_selected.is_some() => {
                        state.reset_tables_widgets();
                    }
                    KeyCode::Esc
                        if state.selected_tab == 2
                            && state.data_files_focus
                                == Some(app::DataFilesFocus::LayoutOutline) =>
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
                            && state.data_files_focus
                                == Some(app::DataFilesFocus::LayoutOutline) =>
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
                            && state.data_files_focus
                                == Some(app::DataFilesFocus::LayoutOutline) =>
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
                            && state.data_files_focus
                                == Some(app::DataFilesFocus::LayoutOutline) =>
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
                                || state.data_files_focus
                                    == Some(app::DataFilesFocus::Versions)) =>
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
                                || state.data_files_focus
                                    == Some(app::DataFilesFocus::Versions)) =>
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
                            && state.data_files_focus
                                == Some(app::DataFilesFocus::LayoutOutline) =>
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
                },
            }
        }
    }
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
