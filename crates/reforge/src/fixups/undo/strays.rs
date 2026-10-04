//! An emitter's own simple-particle fields that a tool nested in its `textureMult`, back on
//! the emitter (#MOD-ARTIFACT-0017).
//!
//! The fields are the ones #GAME-CHANGE-0004 took off the emitter, and the struct they are
//! nested in is the one #GAME-CHANGE-0005 made. No build of that struct's class declared
//! them, so in it they are read by nothing. On the emitter the layout rules find them, which
//! is why this runs before every layout.

use ltk_meta::PropertyValueEnum;
use ltk_meta::property::values;

use crate::fixups::rule::Name;
use crate::fixups::{BinFixup, Classes, FixCx, TargetTypes, rules};
use crate::model::{BinHash, FieldPath, FixupInfo, Phase, Record, StructMut};

/// The class of an emitter.
const EMITTER: &str = "VfxEmitterDefinitionData";
/// The emitter's field that holds the struct. The struct holds it again where a tool wrapped
/// it twice.
const HOLDER: &str = "textureMult";
/// The class of the struct the fields are nested in.
const CLASS: &str = "VfxTextureMultDefinitionData";

/// A stray field taken out of a `textureMult`.
struct Stray<'a> {
    name: &'a Name,
    /// How many structs of the class are between the emitter's and the one it was in.
    depth: usize,
    value: PropertyValueEnum,
}

/// The fixup `vfx-texture-mult-strays`.
#[derive(Debug, Clone)]
pub struct TextureMultStrays {
    emitter: Name,
    holder: Name,
    class: Name,
    /// The emitter's simple-particle fields, by the names they have on the emitter.
    fields: Vec<Name>,
}

impl Default for TextureMultStrays {
    fn default() -> Self {
        Self::new()
    }
}

impl TextureMultStrays {
    /// The fixup, on the fields the rule of #GAME-CHANGE-0004 takes off the emitter.
    pub fn new() -> Self {
        let simple = rules::legacy_simple();
        Self {
            emitter: Name::new(EMITTER),
            holder: Name::new(HOLDER),
            class: Name::new(CLASS),
            fields: simple.sources().into_iter().cloned().collect(),
        }
    }

    /// The fields that are strays in the struct: the ones the target build does not read
    /// there.
    fn strays(&self, types: &TargetTypes<'_>) -> Vec<&Name> {
        let class = self.class.hash();
        let undeclared = |field: &&Name| !types.declares(class, field.hash());
        self.fields.iter().filter(undeclared).collect()
    }

    /// Takes the stray fields out of the struct a value holds and out of each struct of the
    /// class nested under the same field in it, the outermost first.
    fn pull<'a>(&self, value: &mut PropertyValueEnum, strays: &[&'a Name]) -> Vec<Stray<'a>> {
        use PropertyValueEnum as V;
        let (class, holder) = (self.class.hash(), self.holder.hash());
        let mut found = Vec::new();
        let (mut value, mut depth) = (value, 0);
        // A loop and no recursion: the depth is the mod's to choose.
        loop {
            let here = value;
            let inner = match here {
                V::Struct(inner) | V::Embedded(values::Embedded(inner))
                    if inner.class_hash == class =>
                {
                    inner
                }
                _ => break,
            };
            for &name in strays {
                if let Some(value) = inner.properties.shift_remove(&name.hash()) {
                    found.push(Stray { name, depth, value });
                }
            }
            let Some(nested) = inner.properties.get_mut(&holder) else {
                break;
            };
            value = nested;
            depth += 1;
        }
        found
    }

    /// Where a stray was: in the struct `depth` structs inside the one at `outermost`.
    fn place(&self, outermost: &FieldPath, stray: &Stray<'_>) -> FieldPath {
        let (class, holder) = (self.class.hash(), self.holder.hash());
        let nest = |at: FieldPath, _| at.field(class, holder);
        let at = (0..stray.depth).fold(outermost.clone(), nest);
        at.field(class, stray.name.hash())
    }

    /// Has the run's names learn the names of the paths the changes hold.
    fn teach(&self, cx: &mut FixCx<'_>, fields: &[BinHash]) {
        for class in [&self.emitter, &self.class] {
            if let Some(name) = class.spelled() {
                cx.learn_class(name);
            }
        }
        let moved = self.fields.iter().filter(|f| fields.contains(&f.hash()));
        for field in moved.chain([&self.holder]) {
            if let Some(name) = field.spelled() {
                cx.learn_field(name);
            }
        }
    }
}

impl BinFixup for TextureMultStrays {
    fn info(&self) -> &FixupInfo {
        static INFO: FixupInfo = FixupInfo::built_in(
            "vfx-texture-mult-strays",
            "An emitter's own fields a tool nested in textureMult, back on the emitter",
            &[Record::mod_artifact(17)],
            Phase::Undo,
        );
        &INFO
    }

    fn applies(&self, types: &TargetTypes<'_>) -> bool {
        !self.strays(types).is_empty()
    }

    fn classes(&self) -> Classes {
        Classes::Only(vec![self.emitter.hash()])
    }

    fn fix(&self, cx: &mut FixCx<'_>, mut s: StructMut<'_>) {
        let Some(mult) = s.fields_mut().get_mut(&self.holder.hash()) else {
            return;
        };
        let strays = self.strays(cx.types());
        let found = self.pull(mult, &strays);
        if found.is_empty() {
            return;
        }
        let outermost = s.path_of(self.holder.hash());
        let fields: Vec<BinHash> = found.iter().map(|stray| stray.name.hash()).collect();
        for stray in found {
            let from = self.place(&outermost, &stray);
            let field = stray.name.hash();
            // The emitter's own value is the one its build read. A nested copy of a field
            // the emitter holds, or of one already taken from a struct further out, goes.
            if s.fields().contains_key(&field) {
                cx.removed(from, &stray.value);
            } else {
                cx.moved(from, s.path_of(field), &stray.value);
                s.fields_mut().insert(field, stray.value);
            }
        }
        self.teach(cx, &fields);
    }
}
