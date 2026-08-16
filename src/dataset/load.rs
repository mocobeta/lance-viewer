use std::{collections::BTreeMap, error::Error, sync::Arc};

use lance::index::DatasetIndexExt;

use super::*;

pub(crate) fn load_dataset_info(uri: &str) -> Result<DatasetInfo, Box<dyn Error>> {
    let runtime = tokio::runtime::Runtime::new()?;
    Ok(runtime.block_on(load_dataset_info_async(uri))?)
}

pub(crate) fn load_version_details(
    uri: &str,
    version: u64,
) -> Result<VersionDetails, Box<dyn Error>> {
    let runtime = tokio::runtime::Runtime::new()?;
    Ok(runtime.block_on(load_version_details_async(uri, version))?)
}

pub(crate) fn load_index_details(
    uri: &str,
    version: u64,
) -> Result<Vec<IndexInfo>, Box<dyn Error>> {
    let runtime = tokio::runtime::Runtime::new()?;
    Ok(runtime.block_on(load_index_details_async(uri, version))?)
}

async fn load_dataset_info_async(uri: &str) -> lance::Result<DatasetInfo> {
    let dataset = lance::Dataset::open(uri).await?;
    let summary = dataset.manifest().summary();
    let schema = dataset.schema();
    let version = dataset.version_id();
    let versions = dataset.versions().await?;
    let current_manifest = manifest_info(&dataset);
    let latest_version = versions
        .iter()
        .map(|version| version.version)
        .max()
        .unwrap_or(version);
    let indices = dataset.load_indices().await?;
    let index_descriptions = dataset.describe_indices(None).await?;
    let storage_indices = indices
        .iter()
        .map(|index| StorageIndex {
            base_id: index.base_id,
            uuid: index.uuid.to_string(),
            files: index.files.as_ref().map(|files| {
                files
                    .iter()
                    .map(|file| (file.path.clone(), file.size_bytes))
                    .collect()
            }),
        })
        .collect::<Vec<_>>();
    let index_infos = index_infos(&indices, &index_descriptions);
    let index_sizes: Vec<u64> = indices
        .iter()
        .filter_map(|index| index.total_size_bytes())
        .collect();
    let fields = fields_from_dataset(&dataset);
    let storage_layout = storage_layout(
        &dataset,
        &versions,
        &storage_indices,
        uses_reversed_manifest_names(&dataset),
    )
    .await;

    Ok(DatasetInfo {
        uri: dataset.uri().to_string(),
        storage_format: current_manifest.storage_format.clone(),
        writer_version: current_manifest.writer_version.clone(),
        branch: current_manifest.branch.clone(),
        tag: current_manifest.tag.clone(),
        current_version: version,
        manifest_path: current_manifest.path.clone(),
        version_count: versions.len(),
        latest_version,
        stale: latest_version != version,
        timestamp: current_manifest.timestamp.clone(),
        rows: summary.total_rows,
        columns: schema.fields.len(),
        fragments: summary.total_fragments,
        data_files: summary.total_data_files,
        data_size_bytes: summary.total_files_size,
        deletion_files: summary.total_deletion_files,
        deleted_rows: summary.total_deletion_file_rows,
        manifest_size_bytes: current_manifest.size_bytes,
        index_count: indices.len(),
        index_size_bytes: index_sizes.iter().sum(),
        index_size_complete: index_sizes.len() == indices.len(),
        index_cache: BTreeMap::from([(version, Ok(index_infos))]),
        fields: fields.clone(),
        storage_layout,
        versions: versions
            .iter()
            .map(|version| VersionInfo {
                version: version.version,
                timestamp: version.timestamp.to_rfc3339(),
            })
            .collect(),
        manifest_cache: BTreeMap::from([(current_manifest.version, Ok(current_manifest.clone()))]),
        schema_cache: BTreeMap::from([(version, Ok(fields))]),
        file_layout_cache: BTreeMap::new(),
        index_file_layout_cache: BTreeMap::new(),
    })
}

fn index_infos(
    indices: &[lance::table::format::IndexMetadata],
    descriptions: &[Arc<dyn lance::index::IndexDescription>],
) -> Vec<IndexInfo> {
    indices
        .iter()
        .map(|index| {
            let description = descriptions
                .iter()
                .find(|description| description.name() == index.name);
            IndexInfo {
                uuid: index.uuid.to_string(),
                base_id: index.base_id,
                name: index.name.clone(),
                dataset_version: index.dataset_version,
                fields: index.fields.clone(),
                index_version: index.index_version,
                fragment_count: index
                    .fragment_bitmap
                    .as_ref()
                    .map(|fragments| fragments.len()),
                files: index.files.as_ref().map(|files| {
                    files
                        .iter()
                        .map(|file| IndexFileInfo {
                            path: file.path.clone(),
                            size_bytes: file.size_bytes,
                        })
                        .collect()
                }),
                index_type: description.map(|description| description.index_type().to_string()),
                indexed_rows: description.map(|description| description.rows_indexed()),
                segment_count: description.map(|description| description.segments().len()),
                total_size_bytes: description
                    .and_then(|description| description.total_size_bytes()),
            }
        })
        .collect()
}

async fn load_index_details_async(uri: &str, version: u64) -> lance::Result<Vec<IndexInfo>> {
    let dataset = lance::Dataset::open(uri).await?;
    let dataset = dataset.checkout_version(version).await?;
    let indices = dataset.load_indices().await?;
    let index_descriptions = dataset.describe_indices(None).await?;
    Ok(index_infos(&indices, &index_descriptions))
}

async fn load_version_details_async(uri: &str, version: u64) -> lance::Result<VersionDetails> {
    let dataset = lance::Dataset::open(uri).await?;
    let dataset = dataset.checkout_version(version).await?;
    Ok(VersionDetails {
        manifest: manifest_info(&dataset),
        fields: fields_from_dataset(&dataset),
    })
}

fn fields_from_dataset(dataset: &lance::Dataset) -> Vec<FieldInfo> {
    let schema = dataset.schema();
    let paths = schema.field_paths();
    schema
        .fields_pre_order()
        .zip(paths)
        .map(|(field, path)| {
            let mut keys = Vec::new();
            if let Some(position) = field.unenforced_primary_key_position {
                keys.push(format!("PK{position}"));
            }
            if let Some(position) = field.unenforced_clustering_key_position {
                keys.push(format!("CK{position}"));
            }
            FieldInfo {
                id: field.id,
                path,
                data_type: format!("{:?}", field.data_type()),
                nullable: field.nullable,
                keys: keys.join(", "),
                metadata: {
                    let mut metadata = field
                        .metadata
                        .iter()
                        .map(|(key, value)| (key.clone(), value.clone()))
                        .collect::<Vec<_>>();
                    metadata.sort_unstable_by(|left, right| left.0.cmp(&right.0));
                    metadata
                },
            }
        })
        .collect()
}

fn manifest_info(dataset: &lance::Dataset) -> ManifestInfo {
    let manifest = dataset.manifest();
    let storage_format = &manifest.data_storage_format;
    let writer_version = manifest
        .writer_version
        .as_ref()
        .map(|writer| {
            let mut version = format!("{} {}", writer.library, writer.version);
            if let Some(prerelease) = &writer.prerelease {
                version.push('-');
                version.push_str(prerelease);
            }
            if let Some(build_metadata) = &writer.build_metadata {
                version.push('+');
                version.push_str(build_metadata);
            }
            version
        })
        .unwrap_or_else(|| "unknown".to_string());
    ManifestInfo {
        version: dataset.version_id(),
        path: dataset.manifest_location().path.to_string(),
        timestamp: manifest.timestamp().to_rfc3339(),
        size_bytes: dataset.manifest_location().size,
        storage_format: format!("{} {}", storage_format.file_format, storage_format.version),
        writer_version,
        branch: manifest.branch.as_deref().unwrap_or("main").to_string(),
        tag: manifest.tag.as_deref().unwrap_or("none").to_string(),
        fragments: manifest
            .fragments
            .iter()
            .map(|fragment| FragmentInfo {
                id: fragment.id,
                rows: fragment.num_rows().map(|rows| rows as u64),
                physical_rows: fragment.physical_rows.map(|rows| rows as u64),
                data_files: fragment.files.len(),
                data_file_details: fragment
                    .files
                    .iter()
                    .map(|file| DataFileInfo {
                        path: file.path.clone(),
                        format: format!("{}.{}", file.file_major_version, file.file_minor_version),
                        rows: fragment.physical_rows.map(|rows| rows as u64),
                        columns: if file.column_indices.is_empty() {
                            file.fields.len()
                        } else {
                            file.column_indices.len()
                        },
                        size_bytes: file.file_size_bytes.get().map(|size| size.get()),
                    })
                    .collect(),
                deleted_rows: match &fragment.deletion_file {
                    Some(deletion_file) => deletion_file.num_deleted_rows.map(|rows| rows as u64),
                    None => Some(0),
                },
                deletion_file: fragment.deletion_file.as_ref().map(|deletion_file| {
                    DeletionFileInfo {
                        id: deletion_file.id,
                        read_version: deletion_file.read_version,
                        file_type: format!("{:?}", deletion_file.file_type).to_lowercase(),
                    }
                }),
            })
            .collect(),
    }
}
