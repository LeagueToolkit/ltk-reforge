//! The bounds on what one archive may make a run read, and the count of inflated bytes held
//! against them.

/// Bounds on what one archive may make a run read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Caps {
    /// Bytes of one entry, and of one chunk of a packed WAD, inflated.
    pub entry_bytes: u64,
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
    // Measured 2026-10-06: the datasets' largest archive holds 2,033 entries and its largest
    // packed WAD 5,261 chunks; the game's largest WAD at 16.19 holds 90,012 chunks.
    fn default() -> Self {
        Self {
            entry_bytes: 512 << 20,
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
