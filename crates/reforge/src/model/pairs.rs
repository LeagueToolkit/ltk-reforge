//! A map whose keys are no strings, written as the list of its pairs in key order: JSON keys a
//! map by strings alone. Used as `#[serde(with = "crate::model::pairs")]`.

use std::collections::BTreeMap;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Writes the map as its pairs.
pub fn serialize<K: Serialize, V: Serialize, S: Serializer>(
    map: &BTreeMap<K, V>,
    s: S,
) -> Result<S::Ok, S::Error> {
    s.collect_seq(map)
}

/// Reads the map from its pairs.
pub fn deserialize<'de, K, V, D>(d: D) -> Result<BTreeMap<K, V>, D::Error>
where
    K: Deserialize<'de> + Ord,
    V: Deserialize<'de>,
    D: Deserializer<'de>,
{
    let pairs = Vec::<(K, V)>::deserialize(d)?;
    Ok(pairs.into_iter().collect())
}
