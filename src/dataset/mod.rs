mod file_layout;
mod file_layout_outline;
mod load;
mod models;
mod storage;
mod storage_tree;

pub(crate) use file_layout::{load_data_file_layout, load_index_file_layout};
pub(crate) use file_layout_outline::{layout_outline_default_expanded, layout_outline_tree};
pub(crate) use load::{load_dataset_info, load_index_details, load_version_details};
pub(crate) use models::*;
use storage::{data_file_path, index_file_path, storage_layout, uses_reversed_manifest_names};
pub(crate) use storage_tree::storage_root_key;

#[derive(Debug, Clone, PartialEq, Eq)]
struct StorageIndex {
    base_id: Option<u32>,
    uuid: String,
    files: Option<Vec<(String, u64)>>,
}
