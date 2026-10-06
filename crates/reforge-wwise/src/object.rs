//! What a cut reads off a `HIRC` object. The body stays bytes: each reader walks the fields before
//! the one it wants and gives `None` when the body ends first.

use crate::cursor::{Cur, put_var, u32_at, u32s};
use crate::{ACTION_PLAY, kind};

/// One `HIRC` object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Object {
    /// The object's type, one of `kind` when a reader follows it.
    pub kind: u8,
    /// The id other objects and the client name the object by.
    pub id: u32,
    /// The bytes after the id.
    pub body: Vec<u8>,
}

/// What an action reads: its type, its target, and for a play action where in the body the bank
/// id sits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Action {
    /// What the action does to its target; `ACTION_PLAY` plays it.
    pub action_type: u8,
    /// The id of the object the action acts on.
    pub target: u32,
    /// The offset in the body of a play action's bank id.
    pub bank_at: Option<usize>,
}

/// The last version whose events count their actions in four bytes. Later ones write the
/// variable-size count.
const LAST_FIXED_COUNT: u32 = 122;

/// A node's direct parent, at the head of its base parameters: after the initial effects (and,
/// from version 137, the metadata effects) and the override-bus id.
fn node_parent(cur: &mut Cur, version: u32) -> Option<Option<u32>> {
    cur.u8()?; // override parent effects
    let effects = usize::from(cur.u8()?);
    if effects > 0 {
        cur.u8()?; // bypass bits
        cur.take(effects * if version <= 145 { 7 } else { 6 })?;
    }
    if version > 136 {
        cur.u8()?; // override parent metadata
        let effects = usize::from(cur.u8()?);
        cur.take(effects * 6)?;
    }
    if version > 89 && version <= 145 {
        cur.u8()?; // override attachment parameters
    }
    cur.u32()?; // override bus
    let parent = cur.u32()?;
    Some((parent != 0).then_some(parent))
}

impl Object {
    /// A node's direct parent (sounds, containers, mixers, music nodes); `None` when it has none,
    /// is no node, or its fields do not read.
    pub fn parent(&self, version: u32) -> Option<u32> {
        let mut cur = Cur::new(&self.body);
        match self.kind {
            kind::SOUND => {
                let plugin = cur.u32()?;
                cur.take(1 + 4 + 4 + 1)?; // stream type, media id, in-memory size, source bits
                // A source plugin carries its parameters, a codec none.
                if plugin & 0xf == 2 {
                    let len = cur.u32()?;
                    cur.take_u32(len)?;
                }
                node_parent(&mut cur, version)?
            }
            kind::RANDOM | kind::SWITCH | kind::ACTOR_MIXER | kind::LAYER => {
                node_parent(&mut cur, version)?
            }
            kind::MUSIC_SEGMENT | kind::MUSIC_SWITCH | kind::MUSIC_PLAYLIST => {
                if version > 89 {
                    cur.u8()?; // midi flags
                }
                node_parent(&mut cur, version)?
            }
            _ => None,
        }
    }

    /// A sound's clip.
    pub fn media(&self) -> Option<u32> {
        if self.kind != kind::SOUND {
            return None;
        }
        // After the plugin id and the stream type.
        u32_at(&self.body, 5)
    }

    /// What an action does and to which object; `None` for another object or a body cut short.
    pub fn action(&self) -> Option<Action> {
        if self.kind != kind::ACTION {
            return None;
        }
        let mut cur = Cur::new(&self.body);
        let action_type = (cur.u16()? >> 8) as u8;
        let target = cur.u32()?;
        let mut bank_at = None;
        if action_type == ACTION_PLAY {
            cur.u8()?; // the target is a bus
            let props = usize::from(cur.u8()?);
            cur.take(props * 5)?;
            let ranged = usize::from(cur.u8()?);
            cur.take(ranged * 9)?;
            cur.u8()?; // fade curve
            bank_at = Some(cur.pos());
            cur.u32()?;
        }
        Some(Action {
            action_type,
            target,
            bank_at,
        })
    }

    /// Whether the object is a bus, an audio or an auxiliary one.
    pub fn is_bus(&self) -> bool {
        matches!(self.kind, kind::BUS | kind::AUX_BUS)
    }

    /// A play action's bank: the one it expects its target in.
    pub fn play_bank(&self) -> Option<u32> {
        u32_at(&self.body, self.action()?.bank_at?)
    }

    /// Points a play action at another bank; nothing else changes.
    pub fn set_play_bank(&mut self, bank: u32) {
        let Some(at) = self.action().and_then(|a| a.bank_at) else {
            return;
        };
        if let Some(id) = self.body.get_mut(at..).and_then(|b| b.first_chunk_mut()) {
            *id = bank.to_le_bytes();
        }
    }

    /// An event's actions, in order.
    pub fn event_actions(&self, version: u32) -> Option<Vec<u32>> {
        if self.kind != kind::EVENT {
            return None;
        }
        let mut cur = Cur::new(&self.body);
        let count = if version <= LAST_FIXED_COUNT {
            cur.u32()?
        } else {
            cur.var()?
        };
        // The ids are taken whole before any is collected: the count is the file's.
        let ids = cur.take(usize::try_from(count).ok()?.checked_mul(4)?)?;
        Some(u32s(ids).collect())
    }

    /// An event naming these actions, its count written as the version writes counts.
    pub fn event(id: u32, actions: &[u32], version: u32) -> Object {
        let mut body = Vec::new();
        if version <= LAST_FIXED_COUNT {
            body.extend_from_slice(&(actions.len() as u32).to_le_bytes());
        } else {
            put_var(&mut body, actions.len() as u32);
        }
        for a in actions {
            body.extend_from_slice(&a.to_le_bytes());
        }
        Object {
            kind: kind::EVENT,
            id,
            body,
        }
    }
}
