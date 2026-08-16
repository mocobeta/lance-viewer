use crate::{dataset, tabs};

use super::AppState;
use super::state as app;
pub(crate) fn load_dataset_info(uri: &str) -> app::DatasetState {
    match dataset::load_dataset_info(uri) {
        Ok(dataset_info) => app::DatasetState::Loaded(Box::new(dataset_info)),
        Err(error) => app::DatasetState::Unavailable {
            uri: uri.to_string(),
            reason: error.to_string(),
        },
    }
}

pub(crate) fn load_selected_version(state: &mut AppState, version: u64) {
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

pub(crate) fn load_selected_indices(state: &mut AppState, version: u64) {
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

pub(crate) fn open_selected_table_index(
    state: &mut AppState,
    load_indices: impl FnOnce(&mut AppState, u64),
) {
    let Some(version) = state.tables_selected else {
        return;
    };
    load_indices(state, version);
    state.selected_tab = tabs::INDICES_TAB;
    state.focus_selected_table_index();
}

pub(crate) fn load_selected_data_file(state: &mut AppState) {
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

pub(crate) fn load_selected_index_file(state: &mut AppState) {
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
