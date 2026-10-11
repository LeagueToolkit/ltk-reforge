//! One parsed bin and the rows it projects.
//!
//! Copied from LTK Manager's `crates/ltk-manager-bin/src/bin_document/document.rs`. Cut:
//! everything an edit and a save hold (the base bytes, the touched objects, undo and redo,
//! the declared project, the typed names), the object tab's header, a variant laid over its
//! base, and the schema. A `PROP` is read by reforge's reader, which keeps the first object
//! listed under a hash (#GAME-BEHAVIOR-0020).

use ltk_meta::{Bin, BinFile};
use reforge::game::Speller;
use reforge::model::{BinHash, bin};

use super::path::{Node, Trace, descend, parse_steps};
use super::records::{self, TARGET_PATH, record_address, steps_under};
use super::rows::{Child, Names, Segment, children_of, hex, label_of};
use super::types::{
    BinDocumentHandle, BinFileKind, BinHeader, BinRow, BinRows, READ_PAGE, READ_ROW_CAP, RowNode,
};

/// Why a read of a bin answers no rows.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BinReadError {
    /// The bytes do not read as a bin.
    #[error("the bytes do not read as a bin: {0}")]
    Unreadable(String),
    /// No node of the file sits at the address, written `entry:path`.
    #[error("no node at {0}")]
    NodeNotFound(String),
    /// A projected read reaches more rows than one call answers.
    #[error("the read reaches {rows} rows, past {cap}")]
    ReadTooWide {
        /// The rows the read reaches.
        rows: usize,
        /// The most one call answers.
        cap: usize,
    },
}

/// One parsed bin, of either kind.
#[derive(Debug)]
pub struct BinDocument {
    file: BinFile,
}

impl BinDocument {
    /// Parse `bytes` as a bin, a `PROP` or a `PTCH` by its magic.
    pub fn parse(bytes: &[u8]) -> Result<Self, BinReadError> {
        let unreadable = |error: bin::BinError| BinReadError::Unreadable(error.to_string());
        let file = match bytes.get(..4) {
            Some(b"PTCH") => BinFile::Override(bin::parse_patch(bytes).map_err(unreadable)?),
            _ => {
                let parsed = bin::parse(bytes).map_err(unreadable)?;
                let objects = parsed.objects.into_iter().map(|object| object.value);
                let mut prop = Bin::new(objects, parsed.dependencies);
                prop.version = parsed.version;
                BinFile::Prop(prop)
            }
        };
        Ok(Self { file })
    }

    /// The header, and one row per object in file order, then one per object the patch
    /// records target.
    pub fn open(&self, speller: &Speller<'_>) -> BinDocumentHandle {
        BinDocumentHandle {
            header: self.header(),
            rows: self.roots(&Names::new(speller)),
        }
    }

    /// The facts the header row draws.
    pub fn header(&self) -> BinHeader {
        match &self.file {
            BinFile::Prop(bin) => BinHeader {
                kind: BinFileKind::Prop,
                version: Some(bin.version),
                objects: bin.objects.len(),
                dependencies: bin.dependencies.len(),
                patches: 0,
                deleted: 0,
            },
            BinFile::Override(patch) => BinHeader {
                kind: BinFileKind::Patch,
                version: None,
                objects: patch.objects.len(),
                dependencies: 0,
                patches: patch.patches.len(),
                deleted: patch.deleted.len(),
            },
        }
    }

    /// The patch records of the file, in file order. A `PROP` holds none.
    fn records(&self) -> &[ltk_meta::PropertyPatch] {
        match &self.file {
            BinFile::Prop(_) => &[],
            BinFile::Override(patch) => &patch.patches,
        }
    }

    /// One row per object in file order, then one per object the patch records target. A
    /// target keeps the order of its first record.
    fn roots(&self, names: &Names<'_>) -> Vec<BinRow> {
        let objects = self.file.objects().values().map(|object| {
            let (name, unnamed) = names.entry(object.path_hash);
            BinRow {
                entry: hex(object.path_hash),
                path: String::new(),
                label: String::new(),
                node: RowNode::Object,
                name,
                unnamed,
                kind: None,
                value: names.struct_value(object.class_hash, object.properties.len()),
            }
        });
        let targets = records::targets(self.records());
        let targets = targets
            .iter()
            .map(|(&target, positions)| records::target_row(target, positions.len(), names));
        objects.chain(targets).collect()
    }

    /// The rows under one node: `offset` in, at most `limit` of them, and the total.
    ///
    /// `path` is the hash path, empty for the object itself. [`TARGET_PATH`] answers the patch
    /// records `entry` takes, and a record's own path what its value holds. A leaf, a null
    /// struct and an absent optional have no rows under them. Refused when `entry` is no object
    /// and no target of the file, or `path` reaches nothing under it.
    pub fn children(
        &self,
        entry: BinHash,
        path: &str,
        offset: usize,
        limit: usize,
        speller: &Speller<'_>,
    ) -> Result<BinRows, BinReadError> {
        self.children_named(entry, path, offset, limit, &Names::new(speller))
    }

    fn children_named(
        &self,
        entry: BinHash,
        path: &str,
        offset: usize,
        limit: usize,
        names: &Names<'_>,
    ) -> Result<BinRows, BinReadError> {
        let not_found = || BinReadError::NodeNotFound(format!("{}:{path}", hex(entry)));
        if path == TARGET_PATH {
            let targets = records::targets(self.records());
            let positions = targets.get(&entry).ok_or_else(not_found)?;
            let rows = records::records_of(entry, self.records(), positions, offset, limit, names);
            return Ok(rows);
        }
        let (node, trace, base) = self.locate(entry, path).ok_or_else(not_found)?;

        let children = children_of(node);
        let total = children.len();
        let window = children.get(offset..).unwrap_or(&[]).iter().take(limit);
        let parent_label = label_of(&base, &trace, names);
        let entry_hex = hex(entry);
        let rows = window
            .map(|child: &Child<'_>| {
                let segment = Segment::of(*child, path, &parent_label, names);
                BinRow {
                    entry: entry_hex.clone(),
                    path: segment.path,
                    label: format!("{parent_label}{}", segment.readable),
                    node: segment.node,
                    name: segment.name,
                    unnamed: segment.unnamed,
                    kind: Some(segment.value.kind().into()),
                    value: names.value_of(segment.value),
                }
            })
            .collect();

        Ok(BinRows { rows, total })
    }

    /// The rows under each of several nodes, one page each, in the order asked.
    ///
    /// The projected read, which a value row uses in place of one call per node. A path
    /// reaching nothing answers an empty page. Refused when `entry` is no object and no target
    /// of the file, and when the paths together reach more than [`READ_ROW_CAP`] rows.
    pub fn children_each(
        &self,
        entry: BinHash,
        paths: &[String],
        speller: &Speller<'_>,
    ) -> Result<Vec<BinRows>, BinReadError> {
        let names = Names::new(speller);
        let targets = records::targets(self.records());
        if !self.file.objects().contains_key(&entry) && !targets.contains_key(&entry) {
            return Err(BinReadError::NodeNotFound(format!("{}:", hex(entry))));
        }

        // Counted before a row is built, so a call over the cap costs a walk rather than the
        // whole answer it is about to be refused.
        let mut rows = 0;
        for path in paths {
            let under = if path == TARGET_PATH {
                targets.get(&entry).map_or(0, Vec::len)
            } else {
                match self.locate(entry, path) {
                    Some((node, ..)) => children_of(node).len(),
                    None => continue,
                }
            };
            rows += under.min(READ_PAGE);
        }
        if rows > READ_ROW_CAP {
            return Err(BinReadError::ReadTooWide {
                rows,
                cap: READ_ROW_CAP,
            });
        }

        paths
            .iter()
            .map(
                |path| match self.children_named(entry, path, 0, READ_PAGE, &names) {
                    Err(BinReadError::NodeNotFound(_)) => Ok(BinRows {
                        rows: Vec::new(),
                        total: 0,
                    }),
                    answer => answer,
                },
            )
            .collect()
    }

    /// The node a hash path reaches, the trace down to it, and the readable path above.
    ///
    /// Under an object the readable path starts empty. Under a record it starts with the
    /// record's own path, and the record has to target `entry`.
    fn locate(&self, entry: BinHash, path: &str) -> Option<(Node<'_>, Vec<Trace<'_>>, String)> {
        if let Some((index, rest)) = record_address(path) {
            let record = self
                .records()
                .get(index)
                .filter(|record| record.object_hash == entry)?;
            let steps = steps_under(rest)?;
            let (node, trace) = descend(Node::Value(&record.value), &steps)?;
            let base = super::rows::cleaned(record.path.as_str().to_owned());
            return Some((node, trace, base));
        }
        let object = self.file.objects().get(&entry)?;
        let (node, trace) = descend(Node::Object(object), &parse_steps(path)?)?;
        Some((node, trace, String::new()))
    }
}
