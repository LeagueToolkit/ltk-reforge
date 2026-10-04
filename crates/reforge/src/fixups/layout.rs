//! The layout fixups that are code: a change Riot made that a table rule cannot say. Each
//! combines values, chooses a class from the schema or tells one value from another, and each
//! is one type in a file of its own, with its record.

mod birth_translation;
mod default_technique;
mod emission_surface;
mod mesh_align;
mod sampler_fields;
mod spawn_shape;

use std::sync::Arc;

pub use self::birth_translation::BirthTranslation;
pub use self::default_technique::DefaultTechnique;
pub use self::emission_surface::EmissionSurface;
pub use self::mesh_align::MeshAlign;
pub use self::sampler_fields::SamplerFields;
pub use self::spawn_shape::SpawnShape;
use super::{BinFixup, FixCx};

/// The class and field names a fixup states.
struct Stated {
    classes: &'static [&'static str],
    fields: &'static [&'static str],
}

impl Stated {
    /// Has the run's names learn them, so that a path of a change spells a name no table
    /// holds.
    fn teach(&self, cx: &mut FixCx<'_>) {
        for class in self.classes {
            cx.learn_class(class);
        }
        for field in self.fields {
            cx.learn_field(field);
        }
    }
}

/// Every built-in fixup of this file, in no order: the set orders them by phase and build.
pub fn all() -> Vec<Arc<dyn BinFixup>> {
    vec![
        Arc::new(DefaultTechnique),
        Arc::new(MeshAlign),
        Arc::new(SpawnShape),
        Arc::new(BirthTranslation),
        Arc::new(SamplerFields),
        Arc::new(EmissionSurface),
    ]
}
