//! Two indexes of the same builds, when `REFORGE_INDEX_A` and `REFORGE_INDEX_B` name them: an
//! index built whole and one appended to agree in every table, and in their object bytes
//! decoded. Ignored by default. Run with `--ignored`.
//!
//! A table is read as a multiset of its lines, summed as digests, so that neither index is held
//! in memory.

#[expect(
    dead_code,
    reason = "the shared dump, of which this test streams the lines alone"
)]
mod dump;

use std::collections::BTreeMap;

use camino::Utf8PathBuf;
use sha2::{Digest, Sha256};

/// A table's lines: how many, and the sums of their digests' two halves.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct Lines {
    count: u64,
    low: u128,
    high: u128,
}

fn tables(dir: &Utf8PathBuf) -> BTreeMap<String, Lines> {
    let mut tables: BTreeMap<String, Lines> = BTreeMap::new();
    dump::each_line(dir, |table, line| {
        let digest: [u8; 32] = Sha256::digest(line.as_bytes()).into();
        let (low, high) = digest.split_at(16);
        let half = |bytes: &[u8]| u128::from_le_bytes(bytes.try_into().unwrap());
        let lines = tables.entry(table.to_string()).or_default();
        lines.count += 1;
        lines.low = lines.low.wrapping_add(half(low));
        lines.high = lines.high.wrapping_add(half(high));
    });
    tables
}

#[test]
#[ignore = "reads two indexes named by REFORGE_INDEX_A and REFORGE_INDEX_B"]
fn two_indexes_of_the_same_builds_agree() {
    let var = |name: &str| {
        let value = std::env::var(name).unwrap_or_else(|_| panic!("{name} names an index"));
        Utf8PathBuf::from(value)
    };
    let (a, b) = (var("REFORGE_INDEX_A"), var("REFORGE_INDEX_B"));
    let (of_a, of_b) = std::thread::scope(|s| {
        let of_a = s.spawn(|| tables(&a));
        let of_b = s.spawn(|| tables(&b));
        (of_a.join().unwrap(), of_b.join().unwrap())
    });
    for (table, lines) in &of_a {
        let same = of_b.get(table) == Some(lines);
        eprintln!(
            "{table}: {} lines, {}",
            lines.count,
            if same { "the same" } else { "different" }
        );
    }
    assert_eq!(of_a, of_b);
}
