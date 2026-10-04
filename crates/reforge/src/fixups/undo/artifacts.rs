//! Fields that tools write and no game build declared (#MOD-ARTIFACT-0020), removed from the
//! classes they are known on. One rule for each class, so that a caller turns one class off.
//!
//! They run before the strip of what the target build does not declare, so the strip counts
//! none of them among what a rename may have lost.

use crate::fixups::rule::Rule;
use crate::model::Record;

/// The fields of one class that tools write and no game build declared.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KnownArtifacts {
    /// What a caller turns the class's rule on and off by.
    pub id: &'static str,
    /// The class the fields are known on. The same hash on another class is left.
    pub class: &'static str,
    /// The fields: a name, or the hash as `0x1e6ba0c4` where no name is known for it.
    pub fields: &'static [&'static str],
}

/// Every class with fields of #MOD-ARTIFACT-0020, which holds what each field holds in mods.
pub const TABLE: [KnownArtifacts; 4] = [
    KnownArtifacts {
        id: "known-artifacts-value-color",
        class: "ValueColor",
        fields: &["0xecbec3b3", "0x20e61d8a"],
    },
    KnownArtifacts {
        id: "known-artifacts-vfx-emitter",
        class: "VfxEmitterDefinitionData",
        fields: &["0x086379e1", "0x82a515a5", "isLocalSpace"],
    },
    KnownArtifacts {
        id: "known-artifacts-vfx-system",
        class: "VfxSystemDefinitionData",
        fields: &["0xc750ee97"],
    },
    KnownArtifacts {
        id: "known-artifacts-static-material",
        class: "StaticMaterialDef",
        fields: &["0xf3d3de85"],
    },
];

impl KnownArtifacts {
    /// The rule that removes the class's fields. A field the target build has come to declare
    /// on the class is left: a rule removes only what the target does not read.
    pub fn rule(&self) -> Rule {
        Rule::remove_fields(self.class, self.fields.iter().copied())
            .id(self.id)
            .title(format!(
                "{}: fields no game build declared, removed",
                self.class
            ))
            .record(Record::mod_artifact(20))
    }
}

/// The rule of every class of the table.
pub fn rules() -> Vec<Rule> {
    TABLE.iter().map(KnownArtifacts::rule).collect()
}
