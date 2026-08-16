use std::collections::BTreeMap;

use super::{StorageEntry, StorageLayout, StorageNode};

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
