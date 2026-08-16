use std::collections::BTreeSet;

use crate::formatting::format_bytes;

use super::{ByteRange, FileLayoutInfo, FileLayoutKind, LegacyFileLayout, V2FileLayout};

/// Internal representation of LayoutOutlineTree.
pub(crate) struct LayoutOutlineTree {
    pub(crate) nodes: Vec<LayoutOutlineNode>,
    roots: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Internal representation of LayoutOutlineNode.
pub(crate) struct LayoutOutlineNode {
    pub(crate) key: String,
    pub(crate) label: String,
    pub(crate) depth: usize,
    pub(crate) children: Vec<usize>,
    pub(crate) expanded: bool,
}

impl LayoutOutlineTree {
    /// Internal helper for visible nodes.
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

    /// Internal helper for node.
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

/// Internal helper for layout outline default expanded.
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

/// Internal helper for layout outline tree.
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
