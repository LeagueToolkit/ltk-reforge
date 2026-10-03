//! An events bank cut to the events that play some clips.

use std::collections::{BTreeMap, BTreeSet};

use crate::{Bank, Object, kind};

/// An events bank cut to the events that reach some clips (`Bank::cut_to`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cut {
    /// Every object of the bank but the events and actions that reach none of the clips, in
    /// their order; a kept event lists only its kept actions.
    pub objects: Vec<Object>,
    /// The kept events' ids.
    pub events: Vec<u32>,
    /// Sounds under a kept action whose clip is not among the clips: they play nothing.
    pub silent: usize,
}

impl Bank {
    /// The events bank cut to the events that reach any of `clips`, through their actions'
    /// targets and every node under them. Every other object is kept: a node reaches its parent
    /// through the parent's child list, and a bank missing nodes plays nothing (in game).
    pub fn cut_to(&self, clips: &BTreeSet<u32>) -> Cut {
        let version = self.version;
        let parent: BTreeMap<u32, u32> = self
            .objects
            .iter()
            .filter_map(|o| o.parent(version).map(|p| (o.id, p)))
            .collect();
        let mut children: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
        for (c, p) in &parent {
            children.entry(*p).or_default().push(*c);
        }
        // The nodes over an own clip: its sound and every parent up the chain.
        let mut reached: BTreeSet<u32> = BTreeSet::new();
        let own = |o: &&Object| o.media().is_some_and(|m| clips.contains(&m));
        for o in self.objects.iter().filter(own) {
            let mut at = Some(o.id);
            while let Some(id) = at {
                if !reached.insert(id) {
                    break;
                }
                at = parent.get(&id).copied();
            }
        }
        let reaching: BTreeSet<u32> = self
            .objects
            .iter()
            .filter(|o| o.action().is_some_and(|a| reached.contains(&a.target)))
            .map(|o| o.id)
            .collect();
        let mut kept_actions: BTreeSet<u32> = BTreeSet::new();
        let mut events = Vec::new();
        let mut objects = Vec::new();
        for o in &self.objects {
            if o.kind != kind::EVENT {
                objects.push(o.clone());
                continue;
            }
            let mut actions = o.event_actions(version).unwrap_or_default();
            actions.retain(|a| reaching.contains(a));
            if !actions.is_empty() {
                kept_actions.extend(&actions);
                events.push(o.id);
                objects.push(Object::event(o.id, &actions, version));
            }
        }
        objects.retain(|o| o.kind != kind::ACTION || kept_actions.contains(&o.id));
        // Sounds under the kept actions' targets that hold no own clip.
        let mut under: BTreeSet<u32> = BTreeSet::new();
        let mut stack: Vec<u32> = objects
            .iter()
            .filter_map(|o| o.action().map(|a| a.target))
            .collect();
        while let Some(id) = stack.pop() {
            if under.insert(id) {
                stack.extend(children.get(&id).into_iter().flatten());
            }
        }
        let silent = self
            .objects
            .iter()
            .filter(|o| under.contains(&o.id) && o.media().is_some_and(|m| !clips.contains(&m)))
            .count();
        Cut {
            objects,
            events,
            silent,
        }
    }
}
