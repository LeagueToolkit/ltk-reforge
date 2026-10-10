//! The bounds on what one archive may make a run read, and the count of inflated bytes held
//! against them.

use serde::{Deserialize, Serialize};

/// Bounds on what one archive may make a run read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Caps {
    /// Bytes of one entry, inflated.
    pub entry_bytes: u64,
    /// Bytes of one chunk of a packed WAD, decompressed: what a read holds in memory at once
    /// with a store on disk (#ADR-0100).
    pub chunk_bytes: u64,
    /// Entries of the archive.
    pub entries: usize,
    /// Bytes of all entries and chunks together, inflated.
    pub total_bytes: u64,
    /// Chunks of one packed WAD.
    pub packed_chunks: usize,
    /// Bytes of the archive file itself.
    pub archive_bytes: u64,
}

impl Default for Caps {
    // #ADR-0070, measured 2026-10-06. The datasets: the largest archive file is 439,570,870
    // bytes, its one entry a stored packed WAD of 439,570,394; the largest chunk inflates to
    // 304,352,832; one archive inflates to 1,056,182,694 in all; the most entries is 2,033 and
    // the most chunks of a packed WAD 5,261. The game at 16.19: the largest WAD holds 90,012
    // chunks, and its largest champion WAD inflates to some 760 MB. The chunk cap (#ADR-0100):
    // the largest of the 1,491,566 files of `baseline-port-6` (2026-10-10) is that chunk,
    // 304,352,832 bytes, and none is over 512 MiB.
    fn default() -> Self {
        Self {
            entry_bytes: 1 << 30,
            chunk_bytes: 1 << 29,
            entries: 1 << 18,
            total_bytes: 4 << 30,
            packed_chunks: 1 << 18,
            archive_bytes: 4 << 30,
        }
    }
}

/// The inflate budget is spent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("more than {cap} bytes inflated")]
pub(super) struct OverBudget {
    /// The cap on all entries and chunks together.
    pub(super) cap: u64,
}

/// The inflated bytes an archive may still make a run hold.
#[derive(Debug)]
pub(super) struct Budget {
    cap: u64,
    used: u64,
}

impl Budget {
    /// A budget with nothing charged.
    pub(super) fn new(cap: u64) -> Self {
        Self { cap, used: 0 }
    }

    /// The cap on all entries and chunks together.
    pub(super) fn cap(&self) -> u64 {
        self.cap
    }

    /// The bytes that may still be inflated.
    pub(super) fn left(&self) -> u64 {
        self.cap.saturating_sub(self.used)
    }

    /// Charges bytes about to be inflated. A charge past the cap leaves the budget spent.
    pub(super) fn charge(&mut self, bytes: u64) -> Result<(), OverBudget> {
        self.used = self.used.saturating_add(bytes);
        if self.used > self.cap {
            return Err(OverBudget { cap: self.cap });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_charge_past_the_cap_is_refused_and_leaves_nothing() {
        let mut budget = Budget::new(1000);
        assert_eq!(budget.charge(600), Ok(()));
        assert_eq!(budget.left(), 400);
        assert_eq!(budget.charge(400), Ok(()));
        assert_eq!(budget.charge(1), Err(OverBudget { cap: 1000 }));
        assert_eq!(budget.left(), 0);
    }
}
