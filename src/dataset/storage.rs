use std::collections::BTreeMap;

use object_store::path::Path;

use super::*;

pub(super) fn data_file_path(
    dataset: &lance::Dataset,
    base_id: Option<u32>,
    data_file_path: &str,
) -> Path {
    match base_id {
        None => dataset.data_dir().join(data_file_path),
        Some(base_id) => {
            let is_dataset_root = dataset
                .manifest()
                .base_paths
                .get(&base_id)
                .is_some_and(|base| base.is_dataset_root);
            if is_dataset_root {
                Path::from("data").join(data_file_path)
            } else {
                Path::from(data_file_path)
            }
        }
    }
}

pub(super) fn index_file_path(
    dataset: &lance::Dataset,
    base_id: Option<u32>,
    uuid: &str,
    file_path: &str,
) -> Path {
    match base_id {
        None => dataset.indices_dir().join(uuid).join(file_path),
        Some(base_id) => {
            let prefix = if is_dataset_root(dataset, Some(base_id)) {
                Path::from("_indices").join(uuid)
            } else {
                Path::from(uuid)
            };
            prefix.join(file_path)
        }
    }
}

pub(super) async fn storage_layout(
    dataset: &lance::Dataset,
    versions: &[lance::dataset::Version],
    indices: &[StorageIndex],
    reversed_manifest_names: bool,
) -> StorageLayout {
    let mut layout = StorageLayout {
        roots: vec![StorageRoot {
            name: ".".to_string(),
            entries: vec![
                StorageEntry {
                    path: "_versions".to_string(),
                    directory: true,
                    size_bytes: None,
                },
                StorageEntry {
                    path: "data".to_string(),
                    directory: true,
                    size_bytes: None,
                },
            ],
        }],
    };

    let manifest_sizes = storage_directory_sizes(dataset, &dataset.versions_dir()).await;
    let root = &mut layout.roots[0];
    for version in versions {
        let path = version_manifest_path(reversed_manifest_names, version.version);
        let filename = path.rsplit('/').next().unwrap_or(path.as_str());
        let size_bytes = match manifest_sizes.get(filename) {
            Some(size_bytes) => Some(*size_bytes),
            None => storage_file_size(dataset, None, &dataset.versions_dir().join(filename)).await,
        };
        root.entries.push(StorageEntry {
            path,
            directory: false,
            size_bytes,
        });
    }

    if let Some(transaction_file) = &dataset.manifest().transaction_file {
        let size_bytes = storage_file_size(
            dataset,
            None,
            &dataset.transactions_dir().join(transaction_file.as_str()),
        )
        .await;
        root.entries.push(StorageEntry {
            path: transaction_file_display_path(transaction_file),
            directory: false,
            size_bytes,
        });
    }

    for fragment in dataset.manifest().fragments.iter() {
        for data_file in &fragment.files {
            let size_bytes = match data_file.file_size_bytes.get() {
                Some(size) => Some(size.get()),
                None => {
                    storage_file_size(
                        dataset,
                        data_file.base_id,
                        &data_file_path(dataset, data_file.base_id, &data_file.path),
                    )
                    .await
                }
            };
            add_file(
                &mut layout,
                dataset,
                data_file.base_id,
                if is_dataset_root(dataset, data_file.base_id) {
                    format!("data/{}", data_file.path)
                } else {
                    data_file.path.clone()
                },
                size_bytes,
            );
        }

        if let Some(deletion_file) = &fragment.deletion_file {
            let filename = format!(
                "{}-{}-{}.{}",
                fragment.id,
                deletion_file.read_version,
                deletion_file.id,
                deletion_file.file_type.suffix()
            );
            let path = deletion_file_path(dataset, deletion_file.base_id, &filename);
            let size_bytes = storage_file_size(dataset, deletion_file.base_id, &path).await;
            add_file(
                &mut layout,
                dataset,
                deletion_file.base_id,
                deletion_file_display_path(
                    is_dataset_root(dataset, deletion_file.base_id),
                    &filename,
                ),
                size_bytes,
            );
        }
    }

    for index in indices {
        let prefix = if is_dataset_root(dataset, index.base_id) {
            format!("_indices/{}/", index.uuid)
        } else {
            format!("{}/", index.uuid)
        };
        if let Some(files) = &index.files {
            for (file, size_bytes) in files {
                add_file(
                    &mut layout,
                    dataset,
                    index.base_id,
                    format!("{}{}", prefix, file),
                    Some(*size_bytes),
                );
            }
        } else {
            add_directory(
                &mut layout,
                dataset,
                index.base_id,
                prefix.trim_end_matches('/').to_string(),
            );
        }
    }

    layout
}

fn version_manifest_path(reversed_names: bool, version: u64) -> String {
    const DETACHED_VERSION_MASK: u64 = 0x8000_0000_0000_0000;

    let filename = if version & DETACHED_VERSION_MASK != 0 {
        format!("d{version}.manifest")
    } else if reversed_names {
        format!("{:020}.manifest", u64::MAX - version)
    } else {
        format!("{version}.manifest")
    };
    format!("_versions/{filename}")
}

fn is_dataset_root(dataset: &lance::Dataset, base_id: Option<u32>) -> bool {
    base_id
        .and_then(|id| dataset.manifest().base_paths.get(&id))
        .is_none_or(|base| base.is_dataset_root)
}

pub(super) fn uses_reversed_manifest_names(dataset: &lance::Dataset) -> bool {
    const DETACHED_VERSION_MASK: u64 = 0x8000_0000_0000_0000;

    if dataset.version_id() & DETACHED_VERSION_MASK != 0 {
        return false;
    }
    let manifest_path = dataset.manifest_location().path.to_string();
    let current_filename = manifest_path.rsplit('/').next().unwrap_or_default();
    current_filename != format!("{}.manifest", dataset.version_id())
}

fn add_file(
    layout: &mut StorageLayout,
    dataset: &lance::Dataset,
    base_id: Option<u32>,
    path: String,
    size_bytes: Option<u64>,
) {
    add_entry(layout, dataset, base_id, path, false, size_bytes);
}

fn add_directory(
    layout: &mut StorageLayout,
    dataset: &lance::Dataset,
    base_id: Option<u32>,
    path: String,
) {
    add_entry(layout, dataset, base_id, path, true, None);
}

fn add_entry(
    layout: &mut StorageLayout,
    dataset: &lance::Dataset,
    base_id: Option<u32>,
    path: String,
    directory: bool,
    size_bytes: Option<u64>,
) {
    let root_name = base_id
        .and_then(|id| dataset.manifest().base_paths.get(&id))
        .map(|base| {
            format!(
                "{} (base {})",
                base.name.as_deref().unwrap_or(base.path.as_str()),
                base.id
            )
        })
        .unwrap_or_else(|| ".".to_string());
    let root_index =
        if let Some(index) = layout.roots.iter().position(|root| root.name == root_name) {
            index
        } else {
            layout.roots.push(StorageRoot {
                name: root_name,
                entries: Vec::new(),
            });
            layout.roots.len() - 1
        };
    let root = &mut layout.roots[root_index];
    if let Some(entry) = root.entries.iter_mut().find(|entry| entry.path == path) {
        entry.directory |= directory;
    } else {
        root.entries.push(StorageEntry {
            path,
            directory,
            size_bytes,
        });
    }
}

async fn storage_file_size(
    dataset: &lance::Dataset,
    base_id: Option<u32>,
    path: &Path,
) -> Option<u64> {
    dataset
        .object_store(base_id)
        .await
        .ok()?
        .size(path)
        .await
        .ok()
}

async fn storage_directory_sizes(dataset: &lance::Dataset, path: &Path) -> BTreeMap<String, u64> {
    let Some(store) = dataset.object_store(None).await.ok() else {
        return BTreeMap::new();
    };
    let Some(listing) = store.list_with_delimiter(Some(path)).await.ok() else {
        return BTreeMap::new();
    };
    listing
        .objects
        .into_iter()
        .filter_map(|object| {
            object
                .location
                .filename()
                .map(|filename| (filename.to_string(), object.size))
        })
        .collect()
}

fn deletion_file_path(dataset: &lance::Dataset, base_id: Option<u32>, filename: &str) -> Path {
    match base_id {
        None => dataset.deletions_dir().join(filename),
        Some(_) => Path::from(deletion_file_display_path(
            is_dataset_root(dataset, base_id),
            filename,
        )),
    }
}

pub(super) fn deletion_file_display_path(is_dataset_root: bool, filename: &str) -> String {
    if is_dataset_root {
        format!("_deletions/{filename}")
    } else {
        filename.to_string()
    }
}

pub(super) fn transaction_file_display_path(transaction_file: &str) -> String {
    format!("_transactions/{transaction_file}")
}

#[cfg(test)]
mod tests {
    use super::{deletion_file_display_path, transaction_file_display_path};

    #[test]
    fn displays_deletion_files_relative_to_their_base_path() {
        assert_eq!(
            deletion_file_display_path(true, "4-1-2.arrow"),
            "_deletions/4-1-2.arrow"
        );
        assert_eq!(
            deletion_file_display_path(false, "4-1-2.arrow"),
            "4-1-2.arrow"
        );
    }

    #[test]
    fn displays_transaction_files_in_the_transactions_directory() {
        assert_eq!(
            transaction_file_display_path("4-123456.txn"),
            "_transactions/4-123456.txn"
        );
    }
}
