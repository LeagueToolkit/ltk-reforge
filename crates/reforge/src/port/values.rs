//! What a port holds that its report does not, as the snapshot after `plan` keeps it
//! (#ADR-0094): the bin values and the banks `build` writes, each by the content id of the
//! bytes it is written as, in the blob store. A port read from its report and these is the
//! port `plan` gave, for every stage after it.
//!
//! The plan's working state is not kept: the game's objects a reach copies (each unit ships
//! its copies), the paths a reference was pointed to and the counts of former paths, the
//! names under the namespace by group, and the names the layout has taken. No stage after
//! `plan` reads them.

use std::collections::BTreeMap;

use ltk_meta::{BinObject, PropertyValueEnum};
use reforge_wwise::Bank;
use serde::{Deserialize, Serialize};

use super::{Port, PortUnit};
use crate::model::entity::ObjectId;
use crate::model::{BinHash, ByteView, ContentId, bin};
use crate::store::BlobStore;

/// Why a port's values do not reopen.
#[derive(Debug, thiserror::Error)]
pub enum ValuesError {
    /// The store holds nothing under an id the values name.
    #[error("the store holds no bytes under {0:?}")]
    Missing(ContentId),
    /// The bytes do not read as what they were written as, or the values do not fit the port.
    #[error("{0}")]
    Unreadable(String),
}

/// The values of a port its report leaves out, by the content id of their bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortValues {
    /// Each object the port answers for, as a bin of that object alone.
    objects: BTreeMap<ObjectId, ContentId>,
    /// Each unit's copies of the game's objects, in the order of the units, each as a bin
    /// of that object alone.
    copies: Vec<Vec<ContentId>>,
    /// The value of each HUD edit, champion by champion, as a bin of one object holding it.
    edits: Vec<Vec<ContentId>>,
    /// Each bank made, as the build writes it.
    banks: Vec<ContentId>,
}

/// The field and the object a HUD edit's value is kept under: no name of the game's.
const HELD: BinHash = BinHash(0);

fn object_bytes(object: &BinObject) -> ByteView {
    bin::write_one(object).into()
}

fn value_bytes(value: &PropertyValueEnum) -> ByteView {
    let object = BinObject::builder(HELD, HELD)
        .property(HELD, value.clone())
        .build();
    object_bytes(&object)
}

impl PortValues {
    /// The values of a port, their bytes put into `store`.
    pub fn of(port: &Port, store: &dyn BlobStore) -> Self {
        let put = |bytes: ByteView| store.put(bytes);
        let objects = port.objects.iter();
        let units = port.units.iter();
        let champions = port.hud.champions.iter();
        Self {
            objects: objects
                .map(|(&id, object)| (id, put(object_bytes(object))))
                .collect(),
            copies: units
                .map(|unit| unit.copies.iter().map(|o| put(object_bytes(o))).collect())
                .collect(),
            edits: champions
                .map(|hud| {
                    hud.edits
                        .iter()
                        .map(|e| put(value_bytes(&e.value)))
                        .collect()
                })
                .collect(),
            banks: port
                .audio
                .banks
                .iter()
                .map(|made| put(made.bank.write().into()))
                .collect(),
        }
    }

    /// The id of each bank made, in the order of the port's banks.
    pub fn banks(&self) -> &[ContentId] {
        &self.banks
    }

    /// Every id the values name: the objects', the copies', the edits' and the banks'.
    pub fn ids(&self) -> impl Iterator<Item = ContentId> + '_ {
        let objects = self.objects.values().copied();
        let copies = self.copies.iter().flatten().copied();
        let edits = self.edits.iter().flatten().copied();
        objects
            .chain(copies)
            .chain(edits)
            .chain(self.banks.iter().copied())
    }

    /// The port its report gave, with these values read back from `store`.
    pub fn reopen(&self, mut port: Port, store: &dyn BlobStore) -> Result<Port, ValuesError> {
        let bytes = |id: ContentId| store.get(id).ok_or(ValuesError::Missing(id));
        let object = |id: ContentId| -> Result<BinObject, ValuesError> {
            bin::read_one(&bytes(id)?).map_err(|e| unreadable(format!("{id:?}: {e}")))
        };
        port.objects = self
            .objects
            .iter()
            .map(|(&id, &content)| Ok((id, object(content)?)))
            .collect::<Result<_, ValuesError>>()?;
        fit(port.units.len(), self.copies.len(), "units")?;
        for (unit, copies) in port.units.iter_mut().zip(&self.copies) {
            unit.copies = copies
                .iter()
                .map(|&id| object(id))
                .collect::<Result<_, _>>()?;
        }
        fit(port.hud.champions.len(), self.edits.len(), "champions")?;
        for (hud, values) in port.hud.champions.iter_mut().zip(&self.edits) {
            fit(hud.edits.len(), values.len(), "edits")?;
            for (edit, &id) in hud.edits.iter_mut().zip(values) {
                let mut held = object(id)?.properties.into_iter();
                let value = held.next().filter(|(field, _)| *field == HELD);
                edit.value = value
                    .map(|(_, value)| value)
                    .ok_or_else(|| unreadable(format!("{id:?} holds no edit")))?;
            }
        }
        fit(port.audio.banks.len(), self.banks.len(), "banks")?;
        for (made, &id) in port.audio.banks.iter_mut().zip(&self.banks) {
            made.bank = Bank::read(&bytes(id)?).map_err(|e| unreadable(e.to_string()))?;
        }
        Ok(port)
    }
}

impl Port {
    /// The port as its report holds it: without the objects and the copies it answers for,
    /// which the report leaves out and [`PortValues`] keeps.
    pub fn part(&self) -> Port {
        let units = self.units.iter().map(|unit| PortUnit {
            copies: Vec::new(),
            ..unit.clone()
        });
        Port {
            options: self.options.clone(),
            shipping: self.shipping.clone(),
            reach: self.reach.clone(),
            refs: self.refs.clone(),
            layout: self.layout.clone(),
            renames: self.renames.clone(),
            units: units.collect(),
            objects: BTreeMap::new(),
            files: self.files.clone(),
            repointed: self.repointed,
            audio: self.audio.clone(),
            hud: self.hud.clone(),
        }
    }
}

fn unreadable(what: String) -> ValuesError {
    ValuesError::Unreadable(what)
}

/// Refuses values of another count than the port's.
fn fit(port: usize, values: usize, what: &str) -> Result<(), ValuesError> {
    (port == values)
        .then_some(())
        .ok_or_else(|| unreadable(format!("the port has {port} {what}, the values {values}")))
}
