use std::collections::{BTreeMap, BTreeSet};

use crate::dataset::{
    DatasetInfo, layout_outline_default_expanded, layout_outline_tree, storage_root_key,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TablesFocus {
    Versions,
    Details,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DataFilesFocus {
    Versions,
    Fragments,
    DataFiles,
    LayoutOutline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum IndicesFocus {
    Versions,
    Indices,
    Details,
    LayoutOutline,
}

pub(crate) const INDICES_DETAIL_TAB_COUNT: usize = 2;
pub(crate) const TABLES_DETAIL_TAB_COUNT: usize = 4;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DatasetState {
    Empty,
    Loading,
    Loaded(Box<DatasetInfo>),
    Unavailable { uri: String, reason: String },
}

#[derive(Debug)]
pub struct AppState {
    pub light_mode: bool,
    pub selected_tab: usize,
    pub uri: Option<String>,
    pub(crate) dataset_state: DatasetState,
    pub(crate) tables_selected: Option<u64>,
    pub(crate) tables_focus: TablesFocus,
    pub(crate) data_files_focus: Option<DataFilesFocus>,
    pub(crate) data_file_selected: Option<usize>,
    pub(crate) tables_detail_selected: usize,
    pub(crate) schema_selected: Option<usize>,
    pub(crate) fragment_selected: Option<usize>,
    pub(crate) tables_index_selected: Option<usize>,
    pub(crate) storage_expanded: BTreeSet<String>,
    pub(crate) storage_selected: String,
    pub(crate) storage_focus: bool,
    pub(crate) layout_outline_expanded: BTreeMap<(u64, u64, usize), BTreeSet<String>>,
    pub(crate) layout_outline_selected: Option<usize>,
    pub(crate) index_layout_outline_expanded: BTreeMap<(u64, String, String), BTreeSet<String>>,
    pub(crate) index_layout_outline_selected: Option<usize>,
    pub(crate) indices_selected_version: Option<u64>,
    pub(crate) indices_focus: Option<IndicesFocus>,
    pub(crate) index_selected: Option<usize>,
    pub(crate) index_file_selected: Option<usize>,
    pub(crate) indices_detail_selected: usize,
    pub(crate) help_scroll: usize,
    help_max_scroll: usize,
}

impl Default for AppState {
    fn default() -> Self {
        let root_key = storage_root_key(".");
        let storage_expanded = BTreeSet::from([root_key.clone()]);
        Self {
            light_mode: false,
            selected_tab: 0,
            uri: None,
            dataset_state: DatasetState::Empty,
            tables_selected: None,
            tables_focus: TablesFocus::Versions,
            data_files_focus: None,
            data_file_selected: Some(0),
            tables_detail_selected: 0,
            schema_selected: None,
            fragment_selected: None,
            tables_index_selected: None,
            storage_expanded,
            storage_selected: root_key,
            storage_focus: false,
            layout_outline_expanded: BTreeMap::new(),
            layout_outline_selected: None,
            index_layout_outline_expanded: BTreeMap::new(),
            index_layout_outline_selected: None,
            indices_selected_version: None,
            indices_focus: None,
            index_selected: None,
            index_file_selected: None,
            indices_detail_selected: 0,
            help_scroll: 0,
            help_max_scroll: 0,
        }
    }
}

impl AppState {
    pub fn new(uri: Option<String>) -> Self {
        Self {
            uri,
            ..Self::default()
        }
    }

    pub(crate) fn with_dataset_state(uri: Option<String>, dataset_state: DatasetState) -> Self {
        let storage_expanded = match &dataset_state {
            DatasetState::Loaded(dataset_info) => dataset_info.storage_layout.default_expanded(),
            _ => BTreeSet::from([storage_root_key(".")]),
        };
        Self {
            uri,
            dataset_state,
            storage_expanded,
            ..Self::default()
        }
    }

    pub fn toggle_theme(&mut self) {
        self.light_mode = !self.light_mode;
    }

    pub fn previous_tab(&mut self, tab_count: usize) {
        if tab_count > 0 {
            self.selected_tab = self.selected_tab.min(tab_count - 1).saturating_sub(1);
        }
    }

    pub fn next_tab(&mut self, tab_count: usize) {
        if tab_count > 0 {
            self.selected_tab = self.selected_tab.saturating_add(1).min(tab_count - 1);
        }
    }

    pub(crate) fn set_help_max_scroll(&mut self, max_scroll: usize) {
        self.help_max_scroll = max_scroll;
        self.help_scroll = self.help_scroll.min(max_scroll);
    }

    pub(crate) fn scroll_help(&mut self, direction: i8) {
        self.help_scroll = if direction < 0 {
            self.help_scroll.saturating_sub(1)
        } else {
            self.help_scroll.saturating_add(1).min(self.help_max_scroll)
        };
    }

    pub(crate) fn move_tables_selection(&mut self, direction: i8) -> Option<u64> {
        let versions = match &self.dataset_state {
            DatasetState::Loaded(dataset_info) => dataset_info
                .versions
                .iter()
                .rev()
                .map(|version| version.version)
                .collect::<Vec<_>>(),
            _ => return None,
        };
        if versions.is_empty() {
            return None;
        }
        let current = self
            .tables_selected
            .and_then(|selected| versions.iter().position(|version| *version == selected));
        if direction < 0 && current == Some(0) {
            self.tables_selected = None;
            return None;
        }
        let next = match current {
            Some(current) if direction < 0 => current - 1,
            Some(current) => current.saturating_add(1).min(versions.len() - 1),
            None if direction > 0 => 0,
            None => return None,
        };
        self.tables_selected = Some(versions[next]);
        self.layout_outline_selected = None;
        self.tables_selected
    }

    pub(crate) fn move_indices_version_selection(&mut self, direction: i8) -> Option<u64> {
        let versions = match &self.dataset_state {
            DatasetState::Loaded(dataset_info) => dataset_info
                .versions
                .iter()
                .rev()
                .map(|version| version.version)
                .collect::<Vec<_>>(),
            _ => return None,
        };
        if versions.is_empty() {
            return None;
        }
        let current = self
            .indices_selected_version
            .and_then(|selected| versions.iter().position(|version| *version == selected));
        if direction < 0 && current == Some(0) {
            self.indices_selected_version = None;
            self.indices_focus = None;
            self.index_selected = None;
            self.index_file_selected = None;
            return None;
        }
        let next = match current {
            Some(current) if direction < 0 => current - 1,
            Some(current) => current.saturating_add(1).min(versions.len() - 1),
            None if direction > 0 => 0,
            None => return None,
        };
        self.indices_selected_version = Some(versions[next]);
        self.index_selected = None;
        self.index_file_selected = None;
        self.index_layout_outline_selected = None;
        self.indices_selected_version
    }

    pub(crate) fn focus_indices_versions(&mut self) {
        self.indices_focus = Some(IndicesFocus::Versions);
        self.index_selected = None;
        self.index_file_selected = None;
        self.index_layout_outline_selected = None;
    }

    pub(crate) fn reset_indices_widgets(&mut self) {
        self.indices_focus = None;
        self.indices_selected_version = None;
        self.index_selected = None;
        self.index_file_selected = None;
        self.indices_detail_selected = 0;
        self.index_layout_outline_expanded.clear();
        self.index_layout_outline_selected = None;
    }

    pub(crate) fn focus_indices(&mut self) {
        self.indices_focus = Some(IndicesFocus::Indices);
        self.index_selected = None;
        self.index_file_selected = None;
        self.index_layout_outline_selected = None;
        let version = match (&self.dataset_state, self.indices_selected_version) {
            (DatasetState::Loaded(dataset_info), selected) => {
                selected.unwrap_or(dataset_info.current_version)
            }
            _ => return,
        };
        let file_count = match &self.dataset_state {
            DatasetState::Loaded(dataset_info) => dataset_info
                .index_cache
                .get(&version)
                .and_then(|indices| indices.as_ref().ok())
                .map_or(0, Vec::len),
            _ => 0,
        };
        if file_count > 0 {
            self.index_selected = Some(0);
        }
    }

    pub(crate) fn focus_selected_table_index(&mut self) {
        let Some(version) = self.tables_selected else {
            return;
        };
        let selected_index = self.tables_index_selected;
        self.indices_selected_version = Some(version);
        self.focus_indices();
        if selected_index.is_some_and(|selected| selected < self.selected_indices_count()) {
            self.index_selected = selected_index;
        }
    }

    pub(crate) fn focus_index_details(&mut self) {
        self.indices_focus = Some(IndicesFocus::Details);
        self.select_indices_detail(self.indices_detail_selected);
    }

    pub(crate) fn return_to_indices(&mut self) {
        self.indices_focus = Some(IndicesFocus::Indices);
        self.index_file_selected = None;
        self.index_layout_outline_selected = None;
    }

    pub(crate) fn select_indices_detail(&mut self, selected: usize) {
        self.indices_detail_selected = selected.min(INDICES_DETAIL_TAB_COUNT - 1);
        self.index_file_selected = if self.indices_detail_selected == 0 {
            None
        } else if self.selected_index_file_count() > 0 {
            self.index_file_selected
                .filter(|selected| *selected < self.selected_index_file_count())
                .or(Some(0))
        } else {
            None
        };
        self.index_layout_outline_selected = None;
    }

    pub(crate) fn move_indices_detail(&mut self, direction: i8) -> bool {
        let current = self.indices_detail_selected;
        let next = if direction < 0 {
            current.saturating_sub(1)
        } else {
            current.saturating_add(1).min(INDICES_DETAIL_TAB_COUNT - 1)
        };
        if next == current {
            return false;
        }
        self.select_indices_detail(next);
        true
    }

    pub(crate) fn move_index_selection(&mut self, direction: i8) {
        let file_count = self.selected_indices_count();
        if file_count == 0 {
            return;
        }
        let current = self.index_selected;
        if direction < 0 && current == Some(0) {
            self.index_selected = None;
            self.index_file_selected = None;
            self.index_layout_outline_selected = None;
            return;
        }
        let next = match current {
            Some(current) if direction < 0 => current - 1,
            Some(current) => current.saturating_add(1).min(file_count - 1),
            None if direction > 0 => 0,
            None => file_count - 1,
        };
        self.index_selected = Some(next);
        self.index_file_selected = None;
        self.index_layout_outline_selected = None;
    }

    fn selected_indices_count(&self) -> usize {
        match &self.dataset_state {
            DatasetState::Loaded(dataset_info) => {
                let version = self
                    .indices_selected_version
                    .unwrap_or(dataset_info.current_version);
                dataset_info
                    .index_cache
                    .get(&version)
                    .and_then(|indices| indices.as_ref().ok())
                    .map_or(0, Vec::len)
            }
            _ => 0,
        }
    }

    pub(crate) fn move_index_file_selection(&mut self, direction: i8) {
        let file_count = self.selected_index_file_count();
        if file_count == 0 {
            return;
        }
        let current = self.index_file_selected;
        if direction < 0 && current == Some(0) {
            self.index_file_selected = None;
            self.index_layout_outline_selected = None;
            return;
        }
        let next = match current {
            Some(current) if direction < 0 => current - 1,
            Some(current) => current.saturating_add(1).min(file_count - 1),
            None if direction > 0 => 0,
            None => file_count - 1,
        };
        self.index_file_selected = Some(next);
        self.index_layout_outline_selected = None;
    }

    pub(crate) fn focus_index_layout_outline(&mut self) {
        self.indices_focus = Some(IndicesFocus::LayoutOutline);
        if self.index_layout_outline_selected.is_none() {
            self.move_index_layout_outline_selection(1);
        }
    }

    pub(crate) fn move_index_layout_outline_selection(&mut self, direction: i8) {
        let Some((cache_key, layout)) = self.selected_index_file_layout_context() else {
            return;
        };
        let expanded = self.index_layout_outline_expanded.get(&cache_key);
        let tree = layout_outline_tree(&layout, expanded);
        let visible = tree.visible_nodes();
        if visible.is_empty() {
            self.index_layout_outline_selected = None;
            return;
        }
        let current = self
            .index_layout_outline_selected
            .and_then(|selected| visible.iter().position(|id| *id == selected));
        if direction < 0 && current == Some(0) {
            self.index_layout_outline_selected = None;
            return;
        }
        let next = match current {
            Some(current) if direction < 0 => current - 1,
            Some(current) => current.saturating_add(1).min(visible.len() - 1),
            None if direction > 0 => 0,
            None => visible.len() - 1,
        };
        self.index_layout_outline_selected = Some(visible[next]);
    }

    pub(crate) fn toggle_index_layout_outline(&mut self) {
        let Some((cache_key, layout)) = self.selected_index_file_layout_context() else {
            return;
        };
        let expanded = self.index_layout_outline_expanded.get(&cache_key);
        let tree = layout_outline_tree(&layout, expanded);
        let Some(selected) = self.index_layout_outline_selected else {
            return;
        };
        let Some(node) = tree.node(selected) else {
            return;
        };
        if node.children.is_empty() {
            return;
        }
        let node_key = node.key.clone();
        let expanded = self
            .index_layout_outline_expanded
            .entry(cache_key)
            .or_insert_with(layout_outline_default_expanded);
        if !expanded.remove(&node_key) {
            expanded.insert(node_key);
        }
    }

    pub(crate) fn reset_index_layout_outline_selection(&mut self) {
        self.index_layout_outline_selected = None;
    }

    fn selected_index_file_count(&self) -> usize {
        let DatasetState::Loaded(dataset_info) = &self.dataset_state else {
            return 0;
        };
        let version = self
            .indices_selected_version
            .unwrap_or(dataset_info.current_version);
        let Some(index_selected) = self.index_selected else {
            return 0;
        };
        dataset_info
            .index_cache
            .get(&version)
            .and_then(|indices| indices.as_ref().ok())
            .and_then(|indices| indices.get(index_selected))
            .and_then(|index| index.files.as_ref())
            .map_or(0, Vec::len)
    }

    fn selected_index_file_layout_context(
        &self,
    ) -> Option<((u64, String, String), crate::dataset::FileLayoutInfo)> {
        let DatasetState::Loaded(dataset_info) = &self.dataset_state else {
            return None;
        };
        let version = self
            .indices_selected_version
            .unwrap_or(dataset_info.current_version);
        let index_selected = self.index_selected?;
        let file_selected = self.index_file_selected?;
        let indices = dataset_info.index_cache.get(&version)?.as_ref().ok()?;
        let index = indices.get(index_selected)?;
        let file = index.files.as_ref()?.get(file_selected)?;
        let key = (version, index.uuid.clone(), file.path.clone());
        let layout = dataset_info
            .index_file_layout_cache
            .get(&key)?
            .as_ref()
            .ok()?
            .clone();
        Some((key, layout))
    }

    pub(crate) fn focus_details(&mut self) {
        if self.tables_selected.is_some() {
            self.tables_focus = TablesFocus::Details;
            self.select_tables_detail(self.tables_detail_selected);
        }
    }

    pub(crate) fn focus_versions(&mut self) {
        self.tables_focus = TablesFocus::Versions;
        self.schema_selected = None;
        self.fragment_selected = None;
        self.tables_index_selected = None;
        self.layout_outline_selected = None;
    }

    pub(crate) fn reset_tables_widgets(&mut self) {
        self.tables_selected = None;
        self.focus_versions();
        self.tables_detail_selected = 0;
    }

    pub(crate) fn select_tables_detail(&mut self, selected: usize) {
        self.tables_detail_selected = selected.min(TABLES_DETAIL_TAB_COUNT - 1);
        self.data_file_selected = None;
        self.layout_outline_selected = None;
        self.schema_selected = if self.tables_detail_selected == 1 && self.tables_selected.is_some()
        {
            Some(0)
        } else {
            None
        };
        self.fragment_selected =
            if self.tables_detail_selected == 2 && self.tables_selected.is_some() {
                Some(0)
            } else {
                None
            };
        self.tables_index_selected = if self.tables_detail_selected == 3 {
            self.tables_index_selected
                .filter(|selected| *selected < self.tables_index_count())
                .or_else(|| (self.tables_index_count() > 0).then_some(0))
        } else {
            None
        };
    }

    pub(crate) fn move_tables_detail(&mut self, direction: i8) -> bool {
        let current = self.tables_detail_selected;
        let next = if direction < 0 {
            current.saturating_sub(1)
        } else {
            current.saturating_add(1).min(TABLES_DETAIL_TAB_COUNT - 1)
        };
        if next == current {
            return false;
        }
        self.select_tables_detail(next);
        true
    }

    pub(crate) fn move_schema_selection(&mut self, direction: i8) {
        let field_count = match (&self.dataset_state, self.tables_selected) {
            (DatasetState::Loaded(dataset_info), Some(version)) => dataset_info
                .schema_cache
                .get(&version)
                .and_then(|fields| fields.as_ref().ok())
                .map_or(0, Vec::len),
            _ => 0,
        };
        if field_count == 0 {
            return;
        }
        let current = self.schema_selected;
        if direction < 0 && current == Some(0) {
            self.schema_selected = None;
            return;
        }
        let next = match current {
            Some(current) if direction < 0 => current - 1,
            Some(current) => current.saturating_add(1).min(field_count - 1),
            None if direction > 0 => 0,
            None => field_count - 1,
        };
        self.schema_selected = Some(next);
    }

    pub(crate) fn move_fragment_selection(&mut self, direction: i8) {
        let fragment_count = match (&self.dataset_state, self.tables_selected) {
            (DatasetState::Loaded(dataset_info), Some(version)) => dataset_info
                .manifest_cache
                .get(&version)
                .and_then(|manifest| manifest.as_ref().ok())
                .map_or(0, |manifest| manifest.fragments.len()),
            _ => 0,
        };
        if fragment_count == 0 {
            return;
        }
        self.data_file_selected = None;
        self.layout_outline_selected = None;
        let current = self.fragment_selected;
        if direction < 0 && current == Some(0) {
            self.fragment_selected = None;
            return;
        }
        let next = match current {
            Some(current) if direction < 0 => current - 1,
            Some(current) => current.saturating_add(1).min(fragment_count - 1),
            None if direction > 0 => 0,
            None => fragment_count - 1,
        };
        self.fragment_selected = Some(next);
    }

    pub(crate) fn ensure_tables_index_selection(&mut self) {
        if self.tables_detail_selected == 3 && self.tables_focus == TablesFocus::Details {
            self.tables_index_selected = self
                .tables_index_selected
                .filter(|selected| *selected < self.tables_index_count())
                .or_else(|| (self.tables_index_count() > 0).then_some(0));
        }
    }

    pub(crate) fn move_tables_index_selection(&mut self, direction: i8) {
        let index_count = self.tables_index_count();
        if index_count == 0 {
            return;
        }
        let current = self.tables_index_selected;
        if direction < 0 && current == Some(0) {
            self.tables_index_selected = None;
            return;
        }
        let next = match current {
            Some(current) if direction < 0 => current - 1,
            Some(current) => current.saturating_add(1).min(index_count - 1),
            None if direction > 0 => 0,
            None => index_count - 1,
        };
        self.tables_index_selected = Some(next);
    }

    fn tables_index_count(&self) -> usize {
        match (&self.dataset_state, self.tables_selected) {
            (DatasetState::Loaded(dataset_info), Some(version)) => dataset_info
                .index_cache
                .get(&version)
                .and_then(|indices| indices.as_ref().ok())
                .map_or(0, Vec::len),
            _ => 0,
        }
    }

    pub(crate) fn move_data_file_selection(&mut self, direction: i8) {
        let data_file_count = match (&self.dataset_state, self.tables_selected) {
            (DatasetState::Loaded(dataset_info), selected_version) => {
                let version = selected_version.unwrap_or(dataset_info.current_version);
                let fragment_index = self.fragment_selected.unwrap_or(0);
                dataset_info
                    .manifest_cache
                    .get(&version)
                    .and_then(|manifest| manifest.as_ref().ok())
                    .and_then(|manifest| manifest.fragments.get(fragment_index))
                    .map_or(0, |fragment| fragment.data_file_details.len())
            }
            _ => 0,
        };
        if data_file_count == 0 {
            return;
        }
        let current = self.data_file_selected;
        if direction < 0 && current == Some(0) {
            self.data_file_selected = None;
            return;
        }
        let next = match current {
            Some(current) if direction < 0 => current - 1,
            Some(current) => current.saturating_add(1).min(data_file_count - 1),
            None if direction > 0 => 0,
            None => data_file_count - 1,
        };
        self.data_file_selected = Some(next);
        self.layout_outline_selected = None;
    }

    pub(crate) fn focus_layout_outline(&mut self) {
        self.data_files_focus = Some(DataFilesFocus::LayoutOutline);
        if self.layout_outline_selected.is_none() {
            self.move_layout_outline_selection(1);
        }
    }

    pub(crate) fn focus_data_files(&mut self) {
        self.data_files_focus = Some(DataFilesFocus::DataFiles);
        if self.data_file_selected.is_none() {
            self.data_file_selected = Some(0);
        }
    }

    pub(crate) fn return_to_fragments(&mut self) {
        self.data_files_focus = Some(DataFilesFocus::Fragments);
        self.data_file_selected = None;
        self.layout_outline_selected = None;
    }

    pub(crate) fn focus_data_files_fragments(&mut self) {
        self.data_files_focus = Some(DataFilesFocus::Fragments);
        self.data_file_selected = Some(0);
        self.layout_outline_selected = None;
    }

    pub(crate) fn reset_data_files_widgets(&mut self) {
        self.reset_tables_widgets();
        self.data_files_focus = None;
        self.data_file_selected = Some(0);
        self.layout_outline_expanded.clear();
        self.layout_outline_selected = None;
    }

    pub(crate) fn move_layout_outline_selection(&mut self, direction: i8) {
        let Some((cache_key, layout)) = self.selected_layout_context() else {
            return;
        };
        let expanded = self.layout_outline_expanded.get(&cache_key);
        let tree = layout_outline_tree(&layout, expanded);
        let visible = tree.visible_nodes();
        if visible.is_empty() {
            self.layout_outline_selected = None;
            return;
        }
        let current = self
            .layout_outline_selected
            .and_then(|selected| visible.iter().position(|id| *id == selected));
        if direction < 0 && current == Some(0) {
            self.layout_outline_selected = None;
            return;
        }
        let next = match current {
            Some(current) if direction < 0 => current - 1,
            Some(current) => current.saturating_add(1).min(visible.len() - 1),
            None if direction > 0 => 0,
            None => visible.len() - 1,
        };
        self.layout_outline_selected = Some(visible[next]);
    }

    pub(crate) fn toggle_layout_outline(&mut self) {
        let Some((cache_key, layout)) = self.selected_layout_context() else {
            return;
        };
        let expanded = self.layout_outline_expanded.get(&cache_key);
        let tree = layout_outline_tree(&layout, expanded);
        let Some(selected) = self.layout_outline_selected else {
            return;
        };
        let Some(node) = tree.node(selected) else {
            return;
        };
        if node.children.is_empty() {
            return;
        }
        let node_key = node.key.clone();
        let expanded = self
            .layout_outline_expanded
            .entry(cache_key)
            .or_insert_with(layout_outline_default_expanded);
        if !expanded.remove(&node_key) {
            expanded.insert(node_key);
        }
    }

    pub(crate) fn reset_layout_outline_selection(&mut self) {
        self.layout_outline_selected = None;
    }

    fn selected_layout_context(
        &self,
    ) -> Option<((u64, u64, usize), crate::dataset::FileLayoutInfo)> {
        let DatasetState::Loaded(dataset_info) = &self.dataset_state else {
            return None;
        };
        let version = self.tables_selected.unwrap_or(dataset_info.current_version);
        let manifest = dataset_info.manifest_cache.get(&version)?.as_ref().ok()?;
        let fragment = manifest
            .fragments
            .get(self.fragment_selected.unwrap_or(0))?;
        let data_file_index = self.data_file_selected.unwrap_or(0);
        if data_file_index >= fragment.data_file_details.len() {
            return None;
        }
        let key = (version, fragment.id, data_file_index);
        let layout = dataset_info
            .file_layout_cache
            .get(&key)?
            .as_ref()
            .ok()?
            .clone();
        Some((key, layout))
    }

    pub(crate) fn move_storage_selection(&mut self, direction: i8) {
        let keys = match &self.dataset_state {
            DatasetState::Loaded(dataset_info) => dataset_info
                .storage_layout
                .visible_keys(&self.storage_expanded),
            _ => return,
        };
        if keys.is_empty() {
            return;
        }
        let current = keys
            .iter()
            .position(|key| key == &self.storage_selected)
            .unwrap_or(0);
        let next = if direction < 0 {
            current.saturating_sub(1)
        } else {
            current.saturating_add(1).min(keys.len() - 1)
        };
        self.storage_selected = keys[next].clone();
    }

    pub(crate) fn storage_selection_is_at_start(&self) -> bool {
        let keys = match &self.dataset_state {
            DatasetState::Loaded(dataset_info) => dataset_info
                .storage_layout
                .visible_keys(&self.storage_expanded),
            _ => return true,
        };
        keys.first()
            .is_none_or(|first| first == &self.storage_selected)
    }

    pub(crate) fn reset_storage_widgets(&mut self) {
        let root_key = storage_root_key(".");
        self.storage_expanded = match &self.dataset_state {
            DatasetState::Loaded(dataset_info) => dataset_info.storage_layout.default_expanded(),
            _ => BTreeSet::from([root_key.clone()]),
        };
        self.storage_selected = root_key;
        self.storage_focus = false;
    }

    pub(crate) fn toggle_storage_selection(&mut self) {
        let expandable = match &self.dataset_state {
            DatasetState::Loaded(dataset_info) => dataset_info
                .storage_layout
                .is_expandable(&self.storage_selected),
            _ => false,
        };
        if expandable && !self.storage_expanded.remove(&self.storage_selected) {
            self.storage_expanded.insert(self.storage_selected.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{AppState, DataFilesFocus, IndicesFocus, TablesFocus, storage_root_key};
    use std::collections::BTreeSet;

    #[test]
    fn starts_in_dark_mode_on_overview() {
        let state = AppState::default();

        assert!(!state.light_mode);
        assert_eq!(state.selected_tab, 0);
        assert_eq!(state.uri, None);
        assert_eq!(state.data_file_selected, Some(0));
    }

    #[test]
    fn toggles_theme_and_stops_at_tab_boundaries() {
        let mut state = AppState::default();
        let tab_count = crate::tabs::TITLES.len();

        state.toggle_theme();
        state.previous_tab(tab_count);
        assert!(state.light_mode);
        assert_eq!(state.selected_tab, 0);

        state.next_tab(tab_count);
        assert_eq!(state.selected_tab, 1);

        state.selected_tab = tab_count - 1;
        state.next_tab(tab_count);
        assert_eq!(state.selected_tab, tab_count - 1);
    }

    #[test]
    fn scrolls_help_within_viewport_bounds() {
        let mut state = AppState::default();

        state.set_help_max_scroll(2);
        state.scroll_help(1);
        assert_eq!(state.help_scroll, 1);
        state.scroll_help(1);
        state.scroll_help(1);
        assert_eq!(state.help_scroll, 2);
        state.scroll_help(-1);
        state.scroll_help(-1);
        state.scroll_help(-1);
        assert_eq!(state.help_scroll, 0);
    }

    #[test]
    fn resets_data_files_and_indices_widgets_on_return_to_tab_pane() {
        let mut state = AppState::new(None);

        state.data_files_focus = Some(DataFilesFocus::DataFiles);
        state.tables_selected = Some(1);
        state.fragment_selected = Some(1);
        state.data_file_selected = Some(1);
        state
            .layout_outline_expanded
            .insert((1, 2, 3), BTreeSet::from(["root".to_string()]));
        state.layout_outline_selected = Some(1);
        state.reset_data_files_widgets();
        assert_eq!(state.data_files_focus, None);
        assert_eq!(state.tables_selected, None);
        assert_eq!(state.tables_focus, TablesFocus::Versions);
        assert_eq!(state.tables_detail_selected, 0);
        assert_eq!(state.fragment_selected, None);
        assert_eq!(state.data_file_selected, Some(0));
        assert!(state.layout_outline_expanded.is_empty());
        assert_eq!(state.layout_outline_selected, None);

        state.indices_focus = Some(IndicesFocus::Details);
        state.indices_selected_version = Some(1);
        state.index_selected = Some(1);
        state.index_file_selected = Some(1);
        state.indices_detail_selected = 1;
        state.index_layout_outline_expanded.insert(
            (1, "index".to_string(), "file".to_string()),
            BTreeSet::from(["root".to_string()]),
        );
        state.index_layout_outline_selected = Some(1);
        state.reset_indices_widgets();
        assert_eq!(state.indices_focus, None);
        assert_eq!(state.indices_selected_version, None);
        assert_eq!(state.index_selected, None);
        assert_eq!(state.index_file_selected, None);
        assert_eq!(state.indices_detail_selected, 0);
        assert!(state.index_layout_outline_expanded.is_empty());
        assert_eq!(state.index_layout_outline_selected, None);
    }

    #[test]
    fn returns_nested_widgets_to_their_parent() {
        let mut state = AppState::new(None);

        state.data_files_focus = Some(DataFilesFocus::LayoutOutline);
        state.focus_data_files();
        assert_eq!(state.data_files_focus, Some(DataFilesFocus::DataFiles));

        state.indices_focus = Some(IndicesFocus::LayoutOutline);
        state.focus_index_details();
        assert_eq!(state.indices_focus, Some(IndicesFocus::Details));
    }

    #[test]
    fn returns_index_details_to_the_selected_index() {
        let mut state = AppState {
            indices_focus: Some(IndicesFocus::Details),
            index_selected: Some(2),
            index_file_selected: Some(1),
            index_layout_outline_selected: Some(3),
            ..AppState::default()
        };

        state.return_to_indices();

        assert_eq!(state.indices_focus, Some(IndicesFocus::Indices));
        assert_eq!(state.index_selected, Some(2));
        assert_eq!(state.index_file_selected, None);
        assert_eq!(state.index_layout_outline_selected, None);
    }

    #[test]
    fn returns_data_files_to_the_selected_fragment() {
        let mut state = AppState {
            data_files_focus: Some(DataFilesFocus::DataFiles),
            fragment_selected: Some(2),
            data_file_selected: Some(1),
            layout_outline_selected: Some(3),
            ..AppState::default()
        };

        state.return_to_fragments();

        assert_eq!(state.data_files_focus, Some(DataFilesFocus::Fragments));
        assert_eq!(state.fragment_selected, Some(2));
        assert_eq!(state.data_file_selected, None);
        assert_eq!(state.layout_outline_selected, None);
    }

    #[test]
    fn focuses_data_files_fragments_with_the_first_data_file_selected() {
        let mut state = AppState {
            fragment_selected: Some(2),
            data_file_selected: Some(3),
            layout_outline_selected: Some(1),
            ..AppState::default()
        };

        state.focus_data_files_fragments();

        assert_eq!(state.data_files_focus, Some(DataFilesFocus::Fragments));
        assert_eq!(state.fragment_selected, Some(2));
        assert_eq!(state.data_file_selected, Some(0));
        assert_eq!(state.layout_outline_selected, None);
    }

    #[test]
    fn navigates_table_detail_tabs_and_resets_schema_selection() {
        let mut state = AppState {
            tables_selected: Some(1),
            ..AppState::default()
        };

        state.focus_details();
        assert_eq!(state.tables_focus, TablesFocus::Details);
        assert_eq!(state.tables_detail_selected, 0);
        assert_eq!(state.schema_selected, None);

        assert!(state.move_tables_detail(1));
        assert_eq!(state.tables_detail_selected, 1);
        assert_eq!(state.schema_selected, Some(0));

        assert!(state.move_tables_detail(1));
        assert_eq!(state.tables_detail_selected, 2);
        assert_eq!(state.schema_selected, None);
        assert_eq!(state.fragment_selected, Some(0));

        assert!(state.move_tables_detail(1));
        assert_eq!(state.tables_detail_selected, 3);
        assert_eq!(state.fragment_selected, None);

        assert!(state.move_tables_detail(-1));
        assert_eq!(state.tables_detail_selected, 2);
        assert_eq!(state.schema_selected, None);
        assert_eq!(state.fragment_selected, Some(0));

        assert!(state.move_tables_detail(-1));
        assert_eq!(state.tables_detail_selected, 1);
        assert_eq!(state.schema_selected, Some(0));
        assert_eq!(state.fragment_selected, None);

        state.focus_versions();
        assert_eq!(state.tables_focus, TablesFocus::Versions);
        assert_eq!(state.schema_selected, None);
        assert_eq!(state.fragment_selected, None);

        state.focus_details();
        state.reset_tables_widgets();
        assert_eq!(state.tables_selected, None);
        assert_eq!(state.tables_focus, TablesFocus::Versions);
        assert_eq!(state.tables_detail_selected, 0);
    }

    #[test]
    fn resets_storage_widgets_on_return_to_tab_pane() {
        let mut state = AppState::new(None);

        state.storage_focus = true;
        state.storage_selected = "other".to_string();
        state.storage_expanded.clear();
        state.reset_storage_widgets();

        let root_key = storage_root_key(".");
        assert!(!state.storage_focus);
        assert_eq!(state.storage_selected, root_key);
        assert!(state.storage_expanded.contains(&storage_root_key(".")));
    }

    #[test]
    fn navigates_index_detail_tabs() {
        let mut state = AppState::default();

        assert_eq!(state.indices_detail_selected, 0);
        assert!(!state.move_indices_detail(-1));
        assert!(state.move_indices_detail(1));
        assert_eq!(state.indices_detail_selected, 1);
        assert!(!state.move_indices_detail(1));
        assert!(state.move_indices_detail(-1));
        assert_eq!(state.indices_detail_selected, 0);
    }
}
