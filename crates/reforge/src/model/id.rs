//! Typed ids, the arenas that hand them out, and dense side tables keyed by them.
//!
//! Ids are handed out in creation order and every iteration follows it, so anything derived from
//! an arena is deterministic as long as the arena was filled in a deterministic order.

use std::fmt;
use std::marker::PhantomData;

/// An index into an [`Arena<T>`]. Prints as the entity's sigil and number (`%u3`).
pub struct Id<T> {
    raw: u32,
    _t: PhantomData<fn() -> T>,
}

impl<T> Id<T> {
    /// The id of the item at `raw` in its arena.
    ///
    /// # Panics
    ///
    /// When `raw` does not fit 32 bits: no arena of a mod holds that many items.
    pub fn new(raw: usize) -> Self {
        let raw = u32::try_from(raw)
            .unwrap_or_else(|_| panic!("an arena index of {raw} does not fit 32 bits"));
        Self {
            raw,
            _t: PhantomData,
        }
    }

    /// The position in the arena.
    pub fn index(self) -> usize {
        self.raw as usize
    }
}

impl<T> Clone for Id<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for Id<T> {}
impl<T> PartialEq for Id<T> {
    fn eq(&self, other: &Self) -> bool {
        self.raw == other.raw
    }
}
impl<T> Eq for Id<T> {}
impl<T> PartialOrd for Id<T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl<T> Ord for Id<T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.raw.cmp(&other.raw)
    }
}
impl<T> std::hash::Hash for Id<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.raw.hash(state);
    }
}

/// The one-letter sigil an entity prints with.
pub trait Sigil {
    /// The letter after `%` in a printed id.
    const SIGIL: &'static str;
}

impl<T: Sigil> fmt::Display for Id<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "%{}{}", T::SIGIL, self.raw)
    }
}
impl<T: Sigil> fmt::Debug for Id<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}
impl<T: Sigil> serde::Serialize for Id<T> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de, T: Sigil> serde::Deserialize<'de> for Id<T> {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let text = <String as serde::Deserialize>::deserialize(d)?;
        let number = text
            .strip_prefix('%')
            .and_then(|t| t.strip_prefix(T::SIGIL));
        let raw = number.and_then(|n| n.parse::<u32>().ok());
        let raw = raw.ok_or_else(|| {
            serde::de::Error::custom(format_args!("{text:?} is no id of %{}", T::SIGIL))
        })?;
        Ok(Self {
            raw,
            _t: PhantomData,
        })
    }
}

/// A growable vector that hands out typed ids.
pub struct Arena<T> {
    items: Vec<T>,
}

impl<T> Default for Arena<T> {
    fn default() -> Self {
        Self { items: Vec::new() }
    }
}

impl<T> Arena<T> {
    /// Adds an item and returns its id.
    pub fn push(&mut self, item: T) -> Id<T> {
        let id = Id::new(self.items.len());
        self.items.push(item);
        id
    }

    /// How many items the arena holds.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Whether the arena holds nothing.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Every id, in creation order.
    pub fn ids(&self) -> impl Iterator<Item = Id<T>> + use<T> {
        (0..self.items.len()).map(Id::new)
    }

    /// Every item with its id, in creation order.
    pub fn iter(&self) -> impl Iterator<Item = (Id<T>, &T)> {
        self.items
            .iter()
            .enumerate()
            .map(|(i, item)| (Id::new(i), item))
    }

    /// Every item, in creation order.
    pub fn values(&self) -> impl Iterator<Item = &T> {
        self.items.iter()
    }
}

impl<T> std::ops::Index<Id<T>> for Arena<T> {
    type Output = T;
    fn index(&self, id: Id<T>) -> &T {
        &self.items[id.index()]
    }
}
impl<T> std::ops::IndexMut<Id<T>> for Arena<T> {
    fn index_mut(&mut self, id: Id<T>) -> &mut T {
        &mut self.items[id.index()]
    }
}

/// A dense map from the ids of one arena to values, iterated in id order.
pub struct SideTable<K, V> {
    slots: Vec<Option<V>>,
    _k: PhantomData<fn() -> K>,
}

impl<K, V> Default for SideTable<K, V> {
    fn default() -> Self {
        Self {
            slots: Vec::new(),
            _k: PhantomData,
        }
    }
}

impl<K, V> SideTable<K, V> {
    /// Sets the value of `id` and returns the one it had.
    pub fn insert(&mut self, id: Id<K>, value: V) -> Option<V> {
        let i = id.index();
        if self.slots.len() <= i {
            self.slots.resize_with(i + 1, || None);
        }
        self.slots[i].replace(value)
    }

    /// The value of `id`, when it has one.
    pub fn get(&self, id: Id<K>) -> Option<&V> {
        self.slots.get(id.index()).and_then(Option::as_ref)
    }

    /// The value of `id` to change, when it has one.
    pub fn get_mut(&mut self, id: Id<K>) -> Option<&mut V> {
        self.slots.get_mut(id.index()).and_then(Option::as_mut)
    }

    /// Whether `id` has a value.
    pub fn contains(&self, id: Id<K>) -> bool {
        self.get(id).is_some()
    }

    /// Every value with its id, in id order.
    pub fn iter(&self) -> impl Iterator<Item = (Id<K>, &V)> {
        self.slots
            .iter()
            .enumerate()
            .filter_map(|(i, v)| v.as_ref().map(|v| (Id::new(i), v)))
    }

    /// How many ids have a value.
    pub fn len(&self) -> usize {
        self.slots.iter().filter(|v| v.is_some()).count()
    }

    /// Whether no id has a value.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl<K, V: Clone> Clone for SideTable<K, V> {
    fn clone(&self) -> Self {
        Self {
            slots: self.slots.clone(),
            _k: PhantomData,
        }
    }
}

/// A side table as a list of its ids with their values, in id order.
impl<K: Sigil, V: serde::Serialize> serde::Serialize for SideTable<K, V> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_seq(self.iter())
    }
}

impl<'de, K: Sigil, V: serde::Deserialize<'de>> serde::Deserialize<'de> for SideTable<K, V> {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let pairs = <Vec<(Id<K>, V)> as serde::Deserialize>::deserialize(d)?;
        let mut table = Self::default();
        for (id, value) in pairs {
            table.insert(id, value);
        }
        Ok(table)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Thing;
    impl Sigil for Thing {
        const SIGIL: &'static str = "t";
    }

    #[test]
    fn arena_hands_out_ids_in_order() {
        let mut arena = Arena::default();
        let a = arena.push(Thing);
        let b = arena.push(Thing);
        assert!(a < b);
        assert_eq!(a.to_string(), "%t0");
        assert_eq!(b.to_string(), "%t1");
        assert_eq!(arena.ids().collect::<Vec<_>>(), vec![a, b]);
    }

    #[test]
    fn side_table_iterates_in_id_order() {
        let mut table = SideTable::<Thing, &str>::default();
        table.insert(Id::new(3), "c");
        table.insert(Id::new(0), "a");
        assert_eq!(
            table.iter().map(|(_, v)| *v).collect::<Vec<_>>(),
            vec!["a", "c"]
        );
        assert_eq!(table.len(), 2);
        assert!(table.get(Id::new(1)).is_none());
    }
}
