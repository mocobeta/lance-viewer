use std::error::Error;

use lance_file::reader::{CachedFileMetadata, FileReader};
use lance_io::{
    scheduler::{ScanScheduler, SchedulerConfig},
    utils::CachedFileSize,
};
use prost::Message;
use prost_types::Any;

use super::*;

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
