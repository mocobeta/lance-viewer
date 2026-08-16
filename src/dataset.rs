use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    sync::Arc,
};

use lance::index::DatasetIndexExt;
use lance_file::reader::{CachedFileMetadata, FileReader};
use lance_io::{
    scheduler::{ScanScheduler, SchedulerConfig},
    utils::CachedFileSize,
};
use object_store::path::Path;
use prost::Message;
use prost_types::Any;

use crate::formatting::format_bytes;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DatasetInfo {
    pub(crate) uri: String,
    pub(crate) storage_format: String,
    pub(crate) writer_version: String,
    pub(crate) branch: String,
    pub(crate) tag: String,
    pub(crate) current_version: u64,
    pub(crate) manifest_path: String,
    pub(crate) version_count: usize,
    pub(crate) latest_version: u64,
    pub(crate) stale: bool,
    pub(crate) timestamp: String,
    pub(crate) rows: u64,
    pub(crate) columns: usize,
    pub(crate) fragments: u64,
    pub(crate) data_files: u64,
    pub(crate) data_size_bytes: u64,
    pub(crate) deletion_files: u64,
    pub(crate) deleted_rows: u64,
    pub(crate) manifest_size_bytes: Option<u64>,
    pub(crate) index_count: usize,
    pub(crate) index_size_bytes: u64,
    pub(crate) index_size_complete: bool,
    pub(crate) index_cache: BTreeMap<u64, Result<Vec<IndexInfo>, String>>,
    pub(crate) fields: Vec<FieldInfo>,
    pub(crate) storage_layout: StorageLayout,
    pub(crate) versions: Vec<VersionInfo>,
    pub(crate) manifest_cache: BTreeMap<u64, Result<ManifestInfo, String>>,
    pub(crate) schema_cache: BTreeMap<u64, Result<Vec<FieldInfo>, String>>,
    pub(crate) file_layout_cache: BTreeMap<(u64, u64, usize), Result<FileLayoutInfo, String>>,
    pub(crate) index_file_layout_cache:
        BTreeMap<(u64, String, String), Result<FileLayoutInfo, String>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct VersionInfo {
    pub(crate) version: u64,
    pub(crate) timestamp: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct IndexInfo {
    pub(crate) uuid: String,
    pub(crate) base_id: Option<u32>,
    pub(crate) name: String,
    pub(crate) dataset_version: u64,
    pub(crate) fields: Vec<i32>,
    pub(crate) index_version: i32,
    pub(crate) fragment_count: Option<u64>,
    pub(crate) files: Option<Vec<IndexFileInfo>>,
    pub(crate) index_type: Option<String>,
    pub(crate) indexed_rows: Option<u64>,
    pub(crate) segment_count: Option<usize>,
    pub(crate) total_size_bytes: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct IndexFileInfo {
    pub(crate) path: String,
    pub(crate) size_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ManifestInfo {
    pub(crate) version: u64,
    pub(crate) path: String,
    pub(crate) timestamp: String,
    pub(crate) size_bytes: Option<u64>,
    pub(crate) storage_format: String,
    pub(crate) writer_version: String,
    pub(crate) branch: String,
    pub(crate) tag: String,
    pub(crate) fragments: Vec<FragmentInfo>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FragmentInfo {
    pub(crate) id: u64,
    pub(crate) rows: Option<u64>,
    pub(crate) physical_rows: Option<u64>,
    pub(crate) data_files: usize,
    pub(crate) data_file_details: Vec<DataFileInfo>,
    pub(crate) deleted_rows: Option<u64>,
    pub(crate) deletion_file: Option<DeletionFileInfo>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DataFileInfo {
    pub(crate) path: String,
    pub(crate) format: String,
    pub(crate) rows: Option<u64>,
    pub(crate) columns: usize,
    pub(crate) size_bytes: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FileLayoutInfo {
    pub(crate) path: String,
    pub(crate) file_size_bytes: u64,
    pub(crate) kind: FileLayoutKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LayoutOutlineTree {
    pub(crate) nodes: Vec<LayoutOutlineNode>,
    roots: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LayoutOutlineNode {
    pub(crate) key: String,
    pub(crate) label: String,
    pub(crate) depth: usize,
    pub(crate) children: Vec<usize>,
    pub(crate) expanded: bool,
}

impl LayoutOutlineTree {
    pub(crate) fn visible_nodes(&self) -> Vec<usize> {
        fn visit(tree: &LayoutOutlineTree, id: usize, output: &mut Vec<usize>) {
            output.push(id);
            let node = &tree.nodes[id];
            if node.expanded {
                for &child in &node.children {
                    visit(tree, child, output);
                }
            }
        }

        let mut output = Vec::new();
        for &root in &self.roots {
            visit(self, root, &mut output);
        }
        output
    }

    pub(crate) fn node(&self, id: usize) -> Option<&LayoutOutlineNode> {
        self.nodes.get(id)
    }
}

struct LayoutOutlineBuilder {
    tree: LayoutOutlineTree,
    expanded: Option<BTreeSet<String>>,
}

impl LayoutOutlineBuilder {
    fn new(expanded: Option<&BTreeSet<String>>) -> Self {
        Self {
            tree: LayoutOutlineTree {
                nodes: Vec::new(),
                roots: Vec::new(),
            },
            expanded: expanded.cloned(),
        }
    }

    fn add(
        &mut self,
        parent: Option<usize>,
        key: impl Into<String>,
        label: impl Into<String>,
        default_expanded: bool,
    ) -> usize {
        let key = key.into();
        let depth = parent
            .and_then(|parent| self.tree.nodes.get(parent).map(|node| node.depth + 1))
            .unwrap_or(0);
        let expanded = self
            .expanded
            .as_ref()
            .map_or(default_expanded, |expanded| expanded.contains(&key));
        let id = self.tree.nodes.len();
        self.tree.nodes.push(LayoutOutlineNode {
            key,
            label: label.into(),
            depth,
            children: Vec::new(),
            expanded,
        });
        if let Some(parent) = parent {
            self.tree.nodes[parent].children.push(id);
        } else {
            self.tree.roots.push(id);
        }
        id
    }

    fn finish(self) -> LayoutOutlineTree {
        self.tree
    }
}

pub(crate) fn layout_outline_default_expanded() -> BTreeSet<String> {
    BTreeSet::from([
        "root".to_string(),
        "physical".to_string(),
        "columns".to_string(),
        "global".to_string(),
        "legacy/descriptor".to_string(),
        "legacy/pages".to_string(),
    ])
}

pub(crate) fn layout_outline_tree(
    layout: &FileLayoutInfo,
    expanded: Option<&BTreeSet<String>>,
) -> LayoutOutlineTree {
    let mut builder = LayoutOutlineBuilder::new(expanded);
    let root = builder.add(None, "root", "Binary layout", true);
    match &layout.kind {
        FileLayoutKind::V2(v2) => {
            add_v2_outline_nodes(&mut builder, root, layout.file_size_bytes, v2)
        }
        FileLayoutKind::Legacy(legacy) => {
            add_legacy_outline_nodes(&mut builder, root, layout.file_size_bytes, legacy)
        }
    }
    builder.finish()
}

fn add_v2_outline_nodes(
    builder: &mut LayoutOutlineBuilder,
    root: usize,
    file_size: u64,
    v2: &V2FileLayout,
) {
    let physical = builder.add(Some(root), "physical", "Physical regions", true);
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
    for (index, (label, range)) in regions.iter().enumerate() {
        if range.size > 0 {
            builder.add(
                Some(physical),
                format!("physical/{index}"),
                range_label(*label, *range),
                false,
            );
        }
    }

    let columns = builder.add(Some(root), "columns", "Columns", true);
    for (column_index, column) in v2.columns.iter().enumerate() {
        let column_id = builder.add(
            Some(columns),
            format!("columns/{column_index}"),
            format!("Column {column_index}"),
            false,
        );
        builder.add(
            Some(column_id),
            format!("columns/{column_index}/encoding"),
            format!("encoding: {}", column.encoding),
            false,
        );
        builder.add(
            Some(column_id),
            format!("columns/{column_index}/metadata"),
            range_label("metadata", column.metadata),
            false,
        );
        for (buffer_index, buffer) in column.buffers.iter().enumerate() {
            builder.add(
                Some(column_id),
                format!("columns/{column_index}/metadata-buffer/{buffer_index}"),
                range_label(format!("metadata buffer {buffer_index}"), *buffer),
                false,
            );
        }
        let pages = builder.add(
            Some(column_id),
            format!("columns/{column_index}/pages"),
            format!("Pages ({})", column.pages.len()),
            false,
        );
        for (page_index, page) in column.pages.iter().enumerate() {
            let page_id = builder.add(
                Some(pages),
                format!("columns/{column_index}/pages/{page_index}"),
                format!(
                    "Page {page_index}: rows={} priority={}",
                    page.num_rows, page.priority
                ),
                false,
            );
            builder.add(
                Some(page_id),
                format!("columns/{column_index}/pages/{page_index}/encoding"),
                format!("encoding: {}", page.encoding),
                false,
            );
            for (buffer_index, buffer) in page.buffers.iter().enumerate() {
                builder.add(
                    Some(page_id),
                    format!("columns/{column_index}/pages/{page_index}/buffer/{buffer_index}"),
                    range_label(format!("buffer {buffer_index}"), *buffer),
                    false,
                );
            }
        }
    }

    let globals = builder.add(Some(root), "global", "Global buffers", true);
    for (buffer_index, buffer) in v2.global_buffers.iter().enumerate() {
        builder.add(
            Some(globals),
            format!("global/{buffer_index}"),
            range_label(format!("global buffer {buffer_index}"), *buffer),
            false,
        );
    }
}

fn add_legacy_outline_nodes(
    builder: &mut LayoutOutlineBuilder,
    root: usize,
    file_size: u64,
    legacy: &LegacyFileLayout,
) {
    let descriptor = builder.add(
        Some(root),
        "legacy/descriptor",
        "Descriptor and metadata",
        true,
    );
    if legacy.descriptor_size > 0 {
        builder.add(
            Some(descriptor),
            "legacy/descriptor/file",
            range_label(
                "file descriptor",
                ByteRange {
                    offset: 0,
                    size: legacy.descriptor_size,
                },
            ),
            false,
        );
    }
    builder.add(
        Some(descriptor),
        "legacy/descriptor/metadata",
        range_label(
            "metadata",
            ByteRange {
                offset: legacy.metadata_offset,
                size: legacy.footer_start.saturating_sub(legacy.metadata_offset),
            },
        ),
        false,
    );
    let pages = builder.add(
        Some(root),
        "legacy/pages",
        format!("Pages ({})", legacy.pages.len()),
        true,
    );
    for (index, page) in legacy.pages.iter().enumerate() {
        builder.add(
            Some(pages),
            format!("legacy/pages/{index}"),
            range_label(
                format!("field {} batch {}", page.field_id, page.batch),
                page.range,
            ),
            false,
        );
    }
    let statistics = builder.add(
        Some(root),
        "legacy/statistics",
        format!("Statistics ({})", legacy.statistics_pages.len()),
        false,
    );
    for (index, page) in legacy.statistics_pages.iter().enumerate() {
        builder.add(
            Some(statistics),
            format!("legacy/statistics/{index}"),
            range_label(format!("field {} statistics", page.field_id), page.range),
            false,
        );
    }
    let dictionaries = builder.add(
        Some(root),
        "legacy/dictionaries",
        format!("Dictionaries ({})", legacy.dictionary_ranges.len()),
        false,
    );
    for (index, range) in legacy.dictionary_ranges.iter().enumerate() {
        builder.add(
            Some(dictionaries),
            format!("legacy/dictionaries/{index}"),
            range_label(format!("dictionary {index}"), *range),
            false,
        );
    }
    builder.add(
        Some(root),
        "legacy/footer",
        range_label(
            "footer",
            ByteRange {
                offset: legacy.footer_start,
                size: file_size.saturating_sub(legacy.footer_start),
            },
        ),
        false,
    );
}

fn range_label(label: impl Into<String>, range: ByteRange) -> String {
    format!(
        "{}  [{:#x}, {:#x})  {}",
        label.into(),
        range.offset,
        range.end(),
        format_bytes(range.size)
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FileLayoutKind {
    V2(V2FileLayout),
    Legacy(LegacyFileLayout),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct V2FileLayout {
    pub(crate) num_rows: u64,
    pub(crate) footer: V2Footer,
    pub(crate) columns: Vec<ColumnLayoutInfo>,
    pub(crate) global_buffers: Vec<ByteRange>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct V2Footer {
    pub(crate) column_metadata_start: u64,
    pub(crate) cmo_start: u64,
    pub(crate) gbo_start: u64,
    pub(crate) num_global_buffers: u32,
    pub(crate) num_columns: u32,
    pub(crate) major_version: u16,
    pub(crate) minor_version: u16,
    pub(crate) footer_start: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LegacyFileLayout {
    pub(crate) metadata_offset: u64,
    pub(crate) descriptor_size: u64,
    pub(crate) footer_start: u64,
    pub(crate) major_version: i16,
    pub(crate) minor_version: i16,
    pub(crate) page_table_position: u64,
    pub(crate) page_table_size: u64,
    pub(crate) batch_offsets: Vec<i32>,
    pub(crate) pages: Vec<LegacyPage>,
    pub(crate) statistics_page_table_position: Option<u64>,
    pub(crate) statistics_page_table_size: Option<u64>,
    pub(crate) statistics_pages: Vec<LegacyPage>,
    pub(crate) dictionary_ranges: Vec<ByteRange>,
    pub(crate) field_encodings: Vec<(i32, String)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LegacyPage {
    pub(crate) field_id: i32,
    pub(crate) batch: usize,
    pub(crate) range: ByteRange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ByteRange {
    pub(crate) offset: u64,
    pub(crate) size: u64,
}

impl ByteRange {
    pub(crate) fn end(self) -> u64 {
        self.offset.saturating_add(self.size)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ColumnLayoutInfo {
    pub(crate) encoding: String,
    pub(crate) metadata: ByteRange,
    pub(crate) buffers: Vec<ByteRange>,
    pub(crate) pages: Vec<PageLayoutInfo>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PageLayoutInfo {
    pub(crate) num_rows: u64,
    pub(crate) priority: u64,
    pub(crate) encoding: String,
    pub(crate) buffers: Vec<ByteRange>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DeletionFileInfo {
    pub(crate) id: u64,
    pub(crate) read_version: u64,
    pub(crate) file_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct VersionDetails {
    pub(crate) manifest: ManifestInfo,
    pub(crate) fields: Vec<FieldInfo>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StorageLayout {
    pub(crate) roots: Vec<StorageRoot>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StorageRoot {
    pub(crate) name: String,
    pub(crate) entries: Vec<StorageEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StorageEntry {
    pub(crate) path: String,
    pub(crate) directory: bool,
    pub(crate) size_bytes: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StorageNode {
    pub(crate) key: String,
    pub(crate) name: String,
    pub(crate) directory: bool,
    pub(crate) size_bytes: Option<u64>,
    pub(crate) children: BTreeMap<String, StorageNode>,
}

impl StorageLayout {
    pub(crate) fn default_expanded(&self) -> std::collections::BTreeSet<String> {
        let mut expanded = std::collections::BTreeSet::new();
        for root in self.tree() {
            expanded.insert(root.key);
            expanded.extend(
                root.children
                    .values()
                    .filter(|node| node.directory)
                    .map(|node| node.key.clone()),
            );
        }
        expanded
    }

    pub(crate) fn tree(&self) -> Vec<StorageNode> {
        self.roots
            .iter()
            .map(|root| {
                let root_key = storage_root_key(&root.name);
                let mut children = BTreeMap::new();
                for entry in &root.entries {
                    insert_storage_entry(&mut children, &root_key, entry);
                }
                StorageNode {
                    key: root_key,
                    name: root.name.clone(),
                    directory: true,
                    size_bytes: None,
                    children,
                }
            })
            .collect()
    }

    pub(crate) fn visible_keys(
        &self,
        expanded: &std::collections::BTreeSet<String>,
    ) -> Vec<String> {
        let mut keys = Vec::new();
        for root in self.tree() {
            collect_visible_keys(&root, expanded, &mut keys);
        }
        keys
    }

    pub(crate) fn is_expandable(&self, key: &str) -> bool {
        self.tree()
            .into_iter()
            .any(|root| find_node(&root, key).is_some_and(|node| !node.children.is_empty()))
    }
}

pub(crate) fn storage_root_key(name: &str) -> String {
    format!("root:{name}")
}

fn insert_storage_entry(
    children: &mut BTreeMap<String, StorageNode>,
    root_key: &str,
    entry: &StorageEntry,
) {
    let components: Vec<_> = entry
        .path
        .split('/')
        .filter(|component| !component.is_empty())
        .collect();
    let mut path = String::new();
    let mut current = children;
    for (index, component) in components.iter().enumerate() {
        if !path.is_empty() {
            path.push('/');
        }
        path.push_str(component);
        let key = format!("{root_key}/{path}");
        let node = current
            .entry((*component).to_string())
            .or_insert_with(|| StorageNode {
                key,
                name: (*component).to_string(),
                directory: false,
                size_bytes: None,
                children: BTreeMap::new(),
            });
        if index < components.len() - 1 {
            node.directory = true;
        } else {
            node.directory |= entry.directory;
            node.size_bytes = entry.size_bytes;
        }
        current = &mut node.children;
    }
}

fn collect_visible_keys(
    node: &StorageNode,
    expanded: &std::collections::BTreeSet<String>,
    keys: &mut Vec<String>,
) {
    keys.push(node.key.clone());
    if node.directory && expanded.contains(&node.key) {
        for child in node.children.values() {
            collect_visible_keys(child, expanded, keys);
        }
    }
}

fn find_node<'a>(node: &'a StorageNode, key: &str) -> Option<&'a StorageNode> {
    if node.key == key {
        return Some(node);
    }
    node.children
        .values()
        .find_map(|child| find_node(child, key))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StorageIndex {
    base_id: Option<u32>,
    uuid: String,
    files: Option<Vec<(String, u64)>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FieldInfo {
    pub(crate) id: i32,
    pub(crate) path: String,
    pub(crate) data_type: String,
    pub(crate) nullable: bool,
    pub(crate) keys: String,
    pub(crate) metadata: Vec<(String, String)>,
}

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

pub(crate) fn load_data_file_layout(
    uri: &str,
    version: u64,
    fragment_id: u64,
    data_file_index: usize,
) -> Result<FileLayoutInfo, Box<dyn Error>> {
    let runtime = tokio::runtime::Runtime::new()?;
    Ok(runtime.block_on(load_data_file_layout_async(
        uri,
        version,
        fragment_id,
        data_file_index,
    ))?)
}

pub(crate) fn load_index_file_layout(
    uri: &str,
    version: u64,
    uuid: &str,
    base_id: Option<u32>,
    file_path: &str,
) -> Result<FileLayoutInfo, Box<dyn Error>> {
    let runtime = tokio::runtime::Runtime::new()?;
    Ok(runtime.block_on(load_index_file_layout_async(
        uri, version, uuid, base_id, file_path,
    ))?)
}

async fn load_index_file_layout_async(
    uri: &str,
    version: u64,
    uuid: &str,
    base_id: Option<u32>,
    file_path: &str,
) -> lance::Result<FileLayoutInfo> {
    let dataset = lance::Dataset::open(uri)
        .await?
        .checkout_version(version)
        .await?;
    let object_store = dataset.object_store(base_id).await?;
    let path = index_file_path(&dataset, base_id, uuid, file_path);
    let scheduler = ScanScheduler::new(
        object_store.clone(),
        SchedulerConfig::max_bandwidth(&object_store),
    );
    let file_scheduler = scheduler
        .open_file(&path, &CachedFileSize::unknown())
        .await?;
    let metadata = FileReader::read_all_metadata(&file_scheduler).await?;
    let kind = inspect_v2_file(&file_scheduler, &metadata).await?;
    let file_size = file_scheduler.reader().size().await? as u64;

    Ok(FileLayoutInfo {
        path: file_path.to_string(),
        file_size_bytes: file_size,
        kind,
    })
}

async fn load_data_file_layout_async(
    uri: &str,
    version: u64,
    fragment_id: u64,
    data_file_index: usize,
) -> lance::Result<FileLayoutInfo> {
    let dataset = lance::Dataset::open(uri)
        .await?
        .checkout_version(version)
        .await?;
    let fragment = dataset
        .get_fragment(fragment_id as usize)
        .ok_or_else(|| lance::Error::not_found(format!("fragment {fragment_id} not found")))?;
    let data_file = fragment
        .metadata()
        .files
        .get(data_file_index)
        .ok_or_else(|| {
            lance::Error::not_found(format!(
                "data file {data_file_index} not found in fragment {fragment_id}"
            ))
        })?;
    let object_store = dataset.object_store(data_file.base_id).await?;
    let path = data_file_path(&dataset, data_file.base_id, &data_file.path);
    let scheduler = ScanScheduler::new(
        object_store.clone(),
        SchedulerConfig::max_bandwidth(&object_store),
    );
    let file_scheduler = scheduler
        .open_file(&path, &data_file.file_size_bytes)
        .await?;
    let kind = if data_file.file_major_version == 0 && data_file.file_minor_version < 3 {
        inspect_legacy_file(&file_scheduler, &data_file.fields).await?
    } else {
        let metadata = FileReader::read_all_metadata(&file_scheduler).await?;
        inspect_v2_file(&file_scheduler, &metadata).await?
    };
    let file_size_bytes = file_scheduler.reader().size().await? as u64;

    Ok(FileLayoutInfo {
        path: data_file.path.clone(),
        file_size_bytes,
        kind,
    })
}

async fn inspect_v2_file(
    file_scheduler: &lance_io::scheduler::FileScheduler,
    metadata: &CachedFileMetadata,
) -> lance::Result<FileLayoutKind> {
    const FOOTER_SIZE: u64 = 40;
    let file_size = metadata.file_size();
    let footer_start = file_size.checked_sub(FOOTER_SIZE).ok_or_else(|| {
        lance::Error::corrupt_file(
            file_scheduler.reader().path().clone(),
            "file is too small for a V2 footer",
        )
    })?;
    let footer_bytes = file_scheduler
        .submit_single(footer_start..file_size, 0)
        .await?;
    let footer = parse_v2_footer(&footer_bytes, file_size)?;
    let table_bytes = file_scheduler
        .submit_single(footer.cmo_start..footer.footer_start, 0)
        .await?;
    let cmo_size = usize::try_from(footer.gbo_start - footer.cmo_start).map_err(|_| {
        lance::Error::corrupt_file(
            file_scheduler.reader().path().clone(),
            "column metadata offset table is too large",
        )
    })?;
    let column_metadata = parse_offset_table(
        &table_bytes[..cmo_size],
        footer.num_columns,
        "column metadata",
    )?;
    let global_buffers = parse_offset_table(
        &table_bytes[cmo_size..],
        footer.num_global_buffers,
        "global buffer",
    )?;
    if metadata.column_metadatas.len() != column_metadata.len() {
        return Err(lance::Error::corrupt_file(
            file_scheduler.reader().path().clone(),
            format!(
                "footer describes {} columns but {} descriptors were decoded",
                column_metadata.len(),
                metadata.column_metadatas.len()
            ),
        ));
    }
    let columns = metadata
        .column_metadatas
        .iter()
        .zip(column_metadata)
        .map(|(column, metadata)| -> lance::Result<ColumnLayoutInfo> {
            Ok(ColumnLayoutInfo {
                encoding: describe_encoding_safely(column.encoding.as_ref()),
                metadata,
                buffers: byte_ranges(&column.buffer_offsets, &column.buffer_sizes)?,
                pages: column
                    .pages
                    .iter()
                    .map(|page| {
                        Ok(PageLayoutInfo {
                            num_rows: page.length,
                            priority: page.priority,
                            encoding: describe_encoding_safely(page.encoding.as_ref()),
                            buffers: byte_ranges(&page.buffer_offsets, &page.buffer_sizes)?,
                        })
                    })
                    .collect::<lance::Result<Vec<_>>>()?,
            })
        })
        .collect::<lance::Result<Vec<_>>>()?;

    for (column_index, column) in columns.iter().enumerate() {
        validate_range(
            column.metadata,
            file_size,
            &format!("column {column_index} metadata"),
        )?;
        for (buffer_index, buffer) in column.buffers.iter().copied().enumerate() {
            validate_range(
                buffer,
                file_size,
                &format!("column {column_index} metadata buffer {buffer_index}"),
            )?;
        }
        for (page_index, page) in column.pages.iter().enumerate() {
            for (buffer_index, buffer) in page.buffers.iter().copied().enumerate() {
                validate_range(
                    buffer,
                    file_size,
                    &format!("column {column_index} page {page_index} buffer {buffer_index}"),
                )?;
            }
        }
    }
    for (buffer_index, buffer) in global_buffers.iter().copied().enumerate() {
        validate_range(buffer, file_size, &format!("global buffer {buffer_index}"))?;
    }

    Ok(FileLayoutKind::V2(V2FileLayout {
        num_rows: metadata.num_rows,
        footer,
        columns,
        global_buffers,
    }))
}

async fn inspect_legacy_file(
    file_scheduler: &lance_io::scheduler::FileScheduler,
    file_fields: &[i32],
) -> lance::Result<FileLayoutKind> {
    const FOOTER_SIZE: u64 = 16;
    let file_size = file_scheduler.reader().size().await? as u64;
    let footer_start = file_size.checked_sub(FOOTER_SIZE).ok_or_else(|| {
        lance::Error::corrupt_file(
            file_scheduler.reader().path().clone(),
            "file is too small for a legacy footer",
        )
    })?;
    let footer = file_scheduler
        .submit_single(footer_start..file_size, 0)
        .await?;
    if &footer[12..16] != b"LANC" {
        return Err(lance::Error::corrupt_file(
            file_scheduler.reader().path().clone(),
            "invalid legacy footer magic",
        ));
    }
    let metadata_offset = i64::from_le_bytes(footer[0..8].try_into().unwrap());
    if metadata_offset < 0 {
        return Err(lance::Error::corrupt_file(
            file_scheduler.reader().path().clone(),
            "legacy metadata offset is negative",
        ));
    }
    let metadata_offset = metadata_offset as u64;
    if metadata_offset > footer_start {
        return Err(lance::Error::corrupt_file(
            file_scheduler.reader().path().clone(),
            "legacy metadata offset is beyond the footer",
        ));
    }
    let metadata = read_legacy_metadata(file_scheduler, metadata_offset, footer_start).await?;
    let (descriptor, descriptor_size) = read_legacy_descriptor(file_scheduler).await?;
    let pages = read_legacy_pages(file_scheduler, &metadata, file_fields).await?;
    let statistics_pages = read_legacy_statistics_pages(file_scheduler, &metadata).await?;
    let dictionary_ranges = descriptor
        .as_ref()
        .map(|descriptor| {
            descriptor
                .schema
                .as_ref()
                .into_iter()
                .flat_map(|schema| schema.fields.iter())
                .filter(|field| file_fields.contains(&field.id))
                .filter_map(|field| {
                    let dictionary = field.dictionary.as_ref()?;
                    let offset = u64::try_from(dictionary.offset).ok()?;
                    let size = u64::try_from(dictionary.length).ok()?;
                    Some(ByteRange { offset, size })
                })
                .collect()
        })
        .unwrap_or_default();
    let field_encodings = descriptor
        .as_ref()
        .map(|descriptor| {
            descriptor
                .schema
                .as_ref()
                .into_iter()
                .flat_map(|schema| schema.fields.iter())
                .filter(|field| file_fields.contains(&field.id))
                .map(|field| (field.id, legacy_encoding_name(field.encoding)))
                .collect()
        })
        .unwrap_or_default();

    Ok(FileLayoutKind::Legacy(LegacyFileLayout {
        metadata_offset,
        descriptor_size,
        footer_start,
        major_version: i16::from_le_bytes(footer[8..10].try_into().unwrap()),
        minor_version: i16::from_le_bytes(footer[10..12].try_into().unwrap()),
        page_table_position: metadata.page_table_position,
        page_table_size: legacy_page_table_size(&metadata, file_fields),
        batch_offsets: metadata.batch_offsets,
        pages,
        statistics_page_table_position: metadata
            .statistics
            .as_ref()
            .map(|stats| stats.page_table_position),
        statistics_page_table_size: metadata
            .statistics
            .as_ref()
            .map(|stats| (stats.fields.len() as u64) * 16),
        statistics_pages,
        dictionary_ranges,
        field_encodings,
    }))
}

async fn read_legacy_metadata(
    file_scheduler: &lance_io::scheduler::FileScheduler,
    metadata_offset: u64,
    footer_start: u64,
) -> lance::Result<lance_file::format::pb::Metadata> {
    let prefix = file_scheduler
        .submit_single(metadata_offset..metadata_offset + 4, 0)
        .await?;
    let body_size = u64::from(u32::from_le_bytes(prefix[..4].try_into().unwrap()));
    let end = metadata_offset
        .checked_add(4)
        .and_then(|offset| offset.checked_add(body_size))
        .ok_or_else(|| {
            lance::Error::corrupt_file(
                file_scheduler.reader().path().clone(),
                "legacy metadata length overflows",
            )
        })?;
    if end > footer_start {
        return Err(lance::Error::corrupt_file(
            file_scheduler.reader().path().clone(),
            "legacy metadata extends into the footer",
        ));
    }
    let bytes = file_scheduler
        .submit_single(metadata_offset + 4..end, 0)
        .await?;
    lance_file::format::pb::Metadata::decode(bytes.as_ref()).map_err(|error| {
        lance::Error::corrupt_file(
            file_scheduler.reader().path().clone(),
            format!("unable to decode legacy metadata: {error}"),
        )
    })
}

async fn read_legacy_descriptor(
    file_scheduler: &lance_io::scheduler::FileScheduler,
) -> lance::Result<(Option<lance_file::format::pb::FileDescriptor>, u64)> {
    let prefix = file_scheduler.submit_single(0..4, 0).await?;
    let body_size = u64::from(u32::from_le_bytes(prefix[..4].try_into().unwrap()));
    let end = 4_u64.checked_add(body_size).ok_or_else(|| {
        lance::Error::corrupt_file(
            file_scheduler.reader().path().clone(),
            "legacy descriptor length overflows",
        )
    })?;
    let bytes = file_scheduler.submit_single(4..end, 0).await?;
    let descriptor =
        lance_file::format::pb::FileDescriptor::decode(bytes.as_ref()).map_err(|error| {
            lance::Error::corrupt_file(
                file_scheduler.reader().path().clone(),
                format!("unable to decode legacy file descriptor: {error}"),
            )
        })?;
    Ok((Some(descriptor), end))
}

async fn read_legacy_pages(
    file_scheduler: &lance_io::scheduler::FileScheduler,
    metadata: &lance_file::format::pb::Metadata,
    file_fields: &[i32],
) -> lance::Result<Vec<LegacyPage>> {
    let Some(min_field) = file_fields.iter().copied().min() else {
        return Ok(Vec::new());
    };
    let Some(max_field) = file_fields.iter().copied().max() else {
        return Ok(Vec::new());
    };
    let num_batches = metadata.batch_offsets.len().saturating_sub(1);
    if num_batches == 0 || max_field < min_field {
        return Ok(Vec::new());
    }
    let field_count =
        usize::try_from(i64::from(max_field) - i64::from(min_field) + 1).map_err(|_| {
            lance::Error::corrupt_file(
                file_scheduler.reader().path().clone(),
                "legacy field count overflows",
            )
        })?;
    let entries = field_count.checked_mul(num_batches).ok_or_else(|| {
        lance::Error::corrupt_file(
            file_scheduler.reader().path().clone(),
            "legacy page table size overflows",
        )
    })?;
    let byte_count = entries.checked_mul(16).ok_or_else(|| {
        lance::Error::corrupt_file(
            file_scheduler.reader().path().clone(),
            "legacy page table byte size overflows",
        )
    })?;
    let end = metadata
        .page_table_position
        .checked_add(u64::try_from(byte_count).map_err(|_| {
            lance::Error::corrupt_file(
                file_scheduler.reader().path().clone(),
                "legacy page table is too large",
            )
        })?)
        .ok_or_else(|| {
            lance::Error::corrupt_file(
                file_scheduler.reader().path().clone(),
                "legacy page table range overflows",
            )
        })?;
    let bytes = file_scheduler
        .submit_single(metadata.page_table_position..end, 0)
        .await?;
    let mut pages = Vec::new();
    for &field_id in file_fields {
        if field_id < min_field || field_id > max_field {
            continue;
        }
        let field_index = usize::try_from(field_id - min_field).map_err(|_| {
            lance::Error::corrupt_file(
                file_scheduler.reader().path().clone(),
                "legacy field index overflows",
            )
        })?;
        for batch in 0..num_batches {
            let index = (field_index * num_batches + batch) * 16;
            let position = i64::from_le_bytes(bytes[index..index + 8].try_into().unwrap());
            let size = i64::from_le_bytes(bytes[index + 8..index + 16].try_into().unwrap());
            if position < 0 || size < 0 {
                return Err(lance::Error::corrupt_file(
                    file_scheduler.reader().path().clone(),
                    format!("legacy page {field_id}/{batch} has a negative range"),
                ));
            }
            pages.push(LegacyPage {
                field_id,
                batch,
                range: ByteRange {
                    offset: position as u64,
                    size: size as u64,
                },
            });
        }
    }
    Ok(pages)
}

async fn read_legacy_statistics_pages(
    file_scheduler: &lance_io::scheduler::FileScheduler,
    metadata: &lance_file::format::pb::Metadata,
) -> lance::Result<Vec<LegacyPage>> {
    let Some(stats) = metadata.statistics.as_ref() else {
        return Ok(Vec::new());
    };
    let byte_count = stats.fields.len().checked_mul(16).ok_or_else(|| {
        lance::Error::corrupt_file(
            file_scheduler.reader().path().clone(),
            "legacy statistics page table size overflows",
        )
    })?;
    let end = stats
        .page_table_position
        .checked_add(u64::try_from(byte_count).map_err(|_| {
            lance::Error::corrupt_file(
                file_scheduler.reader().path().clone(),
                "legacy statistics table is too large",
            )
        })?)
        .ok_or_else(|| {
            lance::Error::corrupt_file(
                file_scheduler.reader().path().clone(),
                "legacy statistics table range overflows",
            )
        })?;
    let bytes = file_scheduler
        .submit_single(stats.page_table_position..end, 0)
        .await?;
    stats
        .fields
        .iter()
        .enumerate()
        .map(|(index, &field_id)| {
            let offset = index * 16;
            let position = i64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap());
            let size = i64::from_le_bytes(bytes[offset + 8..offset + 16].try_into().unwrap());
            if position < 0 || size < 0 {
                return Err(lance::Error::corrupt_file(
                    file_scheduler.reader().path().clone(),
                    format!("legacy statistics page {field_id} has a negative range"),
                ));
            }
            Ok(LegacyPage {
                field_id,
                batch: 0,
                range: ByteRange {
                    offset: position as u64,
                    size: size as u64,
                },
            })
        })
        .collect()
}

fn legacy_page_table_size(metadata: &lance_file::format::pb::Metadata, file_fields: &[i32]) -> u64 {
    let Some(min_field) = file_fields.iter().copied().min() else {
        return 0;
    };
    let Some(max_field) = file_fields.iter().copied().max() else {
        return 0;
    };
    let fields = i64::from(max_field) - i64::from(min_field) + 1;
    let batches = metadata.batch_offsets.len().saturating_sub(1);
    u64::try_from(fields.max(0))
        .ok()
        .and_then(|fields| fields.checked_mul(batches as u64))
        .and_then(|entries| entries.checked_mul(16))
        .unwrap_or(0)
}

fn parse_v2_footer(bytes: &[u8], file_size: u64) -> lance::Result<V2Footer> {
    const FOOTER_SIZE: usize = 40;
    if bytes.len() != FOOTER_SIZE || &bytes[36..40] != b"LANC" {
        return Err(lance::Error::corrupt_file(
            "data file".into(),
            "invalid V2 footer",
        ));
    }
    let footer_start = file_size.checked_sub(FOOTER_SIZE as u64).ok_or_else(|| {
        lance::Error::corrupt_file("data file".into(), "file is too small for a V2 footer")
    })?;
    let footer = V2Footer {
        column_metadata_start: read_u64(bytes, 0),
        cmo_start: read_u64(bytes, 8),
        gbo_start: read_u64(bytes, 16),
        num_global_buffers: read_u32(bytes, 24),
        num_columns: read_u32(bytes, 28),
        major_version: read_u16(bytes, 32),
        minor_version: read_u16(bytes, 34),
        footer_start,
    };
    if !(footer.column_metadata_start <= footer.cmo_start
        && footer.cmo_start <= footer.gbo_start
        && footer.gbo_start <= footer.footer_start)
    {
        return Err(lance::Error::corrupt_file(
            "data file".into(),
            "V2 footer section offsets are out of order",
        ));
    }
    if footer.gbo_start - footer.cmo_start != u64::from(footer.num_columns) * 16
        || footer.footer_start - footer.gbo_start != u64::from(footer.num_global_buffers) * 16
    {
        return Err(lance::Error::corrupt_file(
            "data file".into(),
            "V2 offset table size does not match footer counts",
        ));
    }
    Ok(footer)
}

fn parse_offset_table(bytes: &[u8], count: u32, label: &str) -> lance::Result<Vec<ByteRange>> {
    let expected_size = usize::try_from(count)
        .ok()
        .and_then(|count| count.checked_mul(16))
        .ok_or_else(|| {
            lance::Error::corrupt_file("data file".into(), format!("{label} table size overflows"))
        })?;
    if bytes.len() != expected_size {
        return Err(lance::Error::corrupt_file(
            "data file".into(),
            format!("{label} table size does not match footer count"),
        ));
    }
    bytes
        .chunks_exact(16)
        .enumerate()
        .map(|(index, entry)| {
            let offset = read_u64(entry, 0);
            let size = read_u64(entry, 8);
            offset.checked_add(size).ok_or_else(|| {
                lance::Error::corrupt_file(
                    "data file".into(),
                    format!("{label} {index} range overflows"),
                )
            })?;
            Ok(ByteRange { offset, size })
        })
        .collect()
}

fn byte_ranges(offsets: &[u64], sizes: &[u64]) -> lance::Result<Vec<ByteRange>> {
    if offsets.len() != sizes.len() {
        return Err(lance::Error::corrupt_file(
            "data file".into(),
            "buffer offset/size count mismatch",
        ));
    }
    offsets
        .iter()
        .zip(sizes)
        .map(|(&offset, &size)| {
            offset.checked_add(size).ok_or_else(|| {
                lance::Error::corrupt_file("data file".into(), "buffer range overflows")
            })?;
            Ok(ByteRange { offset, size })
        })
        .collect()
}

fn validate_range(range: ByteRange, file_size: u64, label: &str) -> lance::Result<()> {
    if range.end() > file_size {
        return Err(lance::Error::corrupt_file(
            "data file".into(),
            format!("{label} range extends beyond file size"),
        ));
    }
    Ok(())
}

fn describe_encoding_safely(encoding: Option<&lance_file::format::pbfile::Encoding>) -> String {
    let Some(encoding) = encoding else {
        return "MISSING".to_string();
    };
    let Some(location) = encoding.location.as_ref() else {
        return "MISSING STYLE".to_string();
    };
    match location {
        lance_file::format::pbfile::encoding::Location::Indirect(indirect) => format!(
            "IndirectEncoding(pos={},size={})",
            indirect.buffer_location, indirect.buffer_length
        ),
        lance_file::format::pbfile::encoding::Location::None(_) => {
            "NoEncodingDescription".to_string()
        }
        lance_file::format::pbfile::encoding::Location::Direct(direct) => {
            describe_direct_encoding(&direct.encoding)
        }
    }
}

fn describe_direct_encoding(bytes: &[u8]) -> String {
    let any = match Any::decode(bytes) {
        Ok(any) => any,
        Err(error) => return format!("InvalidAny(error={error},bytes={})", bytes.len()),
    };
    format!("type_url={} bytes={}", any.type_url, any.value.len())
}

fn legacy_encoding_name(encoding: i32) -> String {
    match encoding {
        0 => "NONE".to_string(),
        1 => "PLAIN".to_string(),
        2 => "VAR_BINARY".to_string(),
        3 => "DICTIONARY".to_string(),
        4 => "RLE".to_string(),
        other => format!("UNKNOWN({other})"),
    }
}

fn read_u64(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap())
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

fn read_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap())
}

fn data_file_path(dataset: &lance::Dataset, base_id: Option<u32>, data_file_path: &str) -> Path {
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

fn index_file_path(
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

async fn storage_layout(
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

fn uses_reversed_manifest_names(dataset: &lance::Dataset) -> bool {
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

fn deletion_file_display_path(is_dataset_root: bool, filename: &str) -> String {
    if is_dataset_root {
        format!("_deletions/{filename}")
    } else {
        filename.to_string()
    }
}

fn transaction_file_display_path(transaction_file: &str) -> String {
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
