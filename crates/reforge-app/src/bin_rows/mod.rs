//! A bin as the rows of a tree, as LTK Manager's bin editor reads one, read-only (#ADR-0104).
//! The window asks for a node's rows a page at a time and never holds the tree. A node is
//! addressed by its object's entry and its hash path, the form a type check's place writes, so
//! a place is a row to reveal.
//!
//! The parsed bins are held in [`BinViews`], a few at a time: a content id names bytes that
//! never change, so a held bin is never stale and one let go is parsed again.

mod document;
mod path;
mod records;
mod rows;
mod types;

use std::collections::VecDeque;
use std::sync::Arc;

use parking_lot::Mutex;
use reforge::game::{NameBook, Names, Speller};
use reforge::model::{BinHash, ContentId, clean};

pub use document::{BinDocument, BinReadError};
pub use records::TARGET_PATH;
pub use types::{
    BinDocumentHandle, BinFileKind, BinHeader, BinRow, BinRows, BinValue, PropertyKind, READ_PAGE,
    READ_ROW_CAP, RowNode,
};

use crate::error::AppError;
use crate::store::Tiers;

/// How many parsed bins [`BinViews`] holds: a few files a person moves between, each parsed
/// once.
pub const HELD_BINS: usize = 8;

/// The bins the viewer read lately, parsed, the latest first.
#[derive(Default)]
pub struct BinViews {
    held: Mutex<VecDeque<(ContentId, Arc<BinDocument>)>>,
}

impl BinViews {
    /// The bin under `content`, held or read from `tiers` and parsed. Refused when no tier
    /// holds the bytes, and when they do not read as a bin.
    pub fn document(
        &self,
        content: ContentId,
        tiers: &Tiers<'_>,
    ) -> Result<Arc<BinDocument>, AppError> {
        if let Some(held) = self.take(content) {
            return Ok(held);
        }
        let bytes = tiers
            .get(content)
            .ok_or_else(|| AppError::BlobMissing { id: content.hex() })?;
        let document = Arc::new(BinDocument::parse(&bytes)?);
        self.hold(content, Arc::clone(&document));
        Ok(document)
    }

    /// The bin held under `content`, moved to the front.
    fn take(&self, content: ContentId) -> Option<Arc<BinDocument>> {
        let mut held = self.held.lock();
        let at = held.iter().position(|(id, _)| *id == content)?;
        let found = held.remove(at)?;
        let document = Arc::clone(&found.1);
        held.push_front(found);
        Some(document)
    }

    fn hold(&self, content: ContentId, document: Arc<BinDocument>) {
        let mut held = self.held.lock();
        held.retain(|(id, _)| *id != content);
        held.push_front((content, document));
        held.truncate(HELD_BINS);
    }
}

impl From<BinReadError> for AppError {
    fn from(error: BinReadError) -> Self {
        match error {
            BinReadError::Unreadable(detail) => AppError::BinUnreadable {
                detail: clean(&detail).into_owned(),
            },
            BinReadError::NodeNotFound(address) => AppError::BinNodeNotFound {
                address: clean(&address).into_owned(),
            },
            BinReadError::ReadTooWide { rows, cap } => AppError::BinReadTooWide { rows, cap },
        }
    }
}

/// An object's entry as the window writes it, `0x` and eight hex digits.
pub fn parse_entry(text: &str) -> Result<BinHash, AppError> {
    text.strip_prefix("0x")
        .filter(|digits| digits.len() == 8)
        .and_then(|digits| u32::from_str_radix(digits, 16).ok())
        .map(BinHash)
        .ok_or_else(|| AppError::Internal {
            detail: format!("{text:?} is no entry hash"),
        })
}

/// Where a run's bins are read: the tiers that hold their bytes, and the names the run learnt.
pub struct BinSource<'a> {
    tiers: Tiers<'a>,
    book: Option<&'a NameBook>,
}

impl<'a> BinSource<'a> {
    /// The bins `tiers` hold, named through `book` where the run has one.
    pub fn new(tiers: Tiers<'a>, book: Option<&'a NameBook>) -> Self {
        Self { tiers, book }
    }

    /// What `ask` reads of the bin under `content`, its hashes spelled through the run's names
    /// and `names`, the tables of the inputs where they are open.
    pub fn read<T>(
        &self,
        views: &BinViews,
        content: ContentId,
        names: Option<&Names>,
        ask: impl FnOnce(&BinDocument, &Speller<'_>) -> Result<T, AppError>,
    ) -> Result<T, AppError> {
        let document = views.document(content, &self.tiers)?;
        let (no_book, no_names) = (NameBook::default(), Names::default());
        let speller = Speller::new(self.book.unwrap_or(&no_book), names.unwrap_or(&no_names));
        ask(&document, &speller)
    }
}
