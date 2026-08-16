use std::collections::BTreeMap;

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FieldInfo {
    pub(crate) id: i32,
    pub(crate) path: String,
    pub(crate) data_type: String,
    pub(crate) nullable: bool,
    pub(crate) keys: String,
    pub(crate) metadata: Vec<(String, String)>,
}
