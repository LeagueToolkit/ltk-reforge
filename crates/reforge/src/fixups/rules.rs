//! The built-in table rules: the fields of an emitter Riot gathered into a struct of their
//! own, each table read off Riot's own conversion across the build that made it. The record
//! of each rule holds the table and its evidence.

use super::rule::Rule;
use crate::model::{Record, Version};

/// The class of an emitter.
const EMITTER: &str = "VfxEmitterDefinitionData";

/// An emitter's filters, in `Filtering` (#GAME-CHANGE-0002).
pub fn filtering() -> Rule {
    Rule::move_fields(EMITTER)
        .into_field("Filtering", "VfxEmitterFiltering")
        .keep([
            "keywordsExcluded",
            "keywordsRequired",
            "spectatorPolicy",
            "censorPolicy",
        ])
        .id("vfx-filtering")
        .title("An emitter's filters, in Filtering")
        .at_build(Version::new(12, 17))
        .record(Record::game_change(2))
}

/// An emitter's linger fields, in `Linger` (#GAME-CHANGE-0003).
pub fn linger() -> Rule {
    Rule::move_fields(EMITTER)
        .into_field("Linger", "VfxLingerDefinitionData")
        .rename("lingerDrag", "KeyedLingerDrag")
        .rename("UseLingerDrag", "UseKeyedLingerDrag")
        .rename("lingerColor", "SeparateLingerColor")
        .rename("useLingerColor", "UseSeparateLingerColor")
        .rename("lingerVelocity", "KeyedLingerVelocity")
        .rename("UseLingerVelocity", "UseKeyedLingerVelocity")
        .rename("lingerAcceleration", "KeyedLingerAcceleration")
        .rename("UseLingerAcceleration", "UseKeyedLingerAcceleration")
        .rename("LingerScale0", "LingerScale")
        .keep(["UseLingerScale"])
        .rename("LingerRotation0", "LingerRotation")
        .keep(["UseLingerRotation"])
        .id("vfx-linger")
        .title("An emitter's linger fields, in Linger")
        .at_build(Version::new(12, 18))
        .record(Record::game_change(3))
}

/// An emitter's scales by its bound object, in `FlexShapeDefinition` (#GAME-CHANGE-0016).
pub fn flex_shape() -> Rule {
    Rule::move_fields(EMITTER)
        .into_field("FlexShapeDefinition", "VfxFlexShapeDefinitionData")
        .keep([
            "scaleEmitOffsetByBoundObjectSize",
            "scaleBirthScaleByBoundObjectSize",
            "scaleBirthScaleByBoundObjectHeight",
            "scaleBirthScaleByBoundObjectRadius",
        ])
        .id("vfx-flex-shape")
        .title("An emitter's scales by its bound object, in FlexShapeDefinition")
        .at_build(Version::new(12, 6))
        .record(Record::game_change(16))
}

/// An emitter's simple-particle fields, in `LegacySimple` (#GAME-CHANGE-0004).
pub fn legacy_simple() -> Rule {
    Rule::move_fields(EMITTER)
        .into_field("LegacySimple", "VfxEmitterLegacySimple")
        .rename("birthScale1", "birthScale")
        .rename("scale1", "scale")
        .rename("birthRotation1", "birthRotation")
        .rename("rotation1", "rotation")
        .rename("birthRotationalVelocity1", "birthRotationalVelocity")
        .rename("uvScrollRate1", "uvScrollRate")
        .keep([
            "particleBind",
            "orientation",
            "scaleBias",
            "hasFixedOrbit",
            "scaleUpFromOrigin",
        ])
        .id("vfx-legacy-simple")
        .title("An emitter's simple-particle fields, in LegacySimple")
        .at_build(Version::new(12, 19))
        .record(Record::game_change(4))
}

/// An emitter's second texture and its fields, in `textureMult` (#GAME-CHANGE-0005).
pub fn texture_mult() -> Rule {
    Rule::move_fields(EMITTER)
        .into_field("textureMult", "VfxTextureMultDefinitionData")
        .keep([
            "textureMult",
            "birthUvScrollRateMult",
            "uvScaleMult",
            "birthUVOffsetMult",
        ])
        .rename("particleUVScrollRateMult", "ParticleIntegratedUvScrollMult")
        .rename("particleUVRotateRateMult", "ParticleIntegratedUvRotateMult")
        .keep([
            "texAddressModeMult",
            "texDivMult",
            "birthUvRotateRateMult",
            "uvScrollClampMult",
            "UvRotationMult",
            "uvScrollAlphaMult",
            "uvTransformCenterMult",
            "emitterUvScrollRateMult",
            "flexBirthUVScrollRateMult",
            "isRandomStartFrameMult",
        ])
        .rename("TextureMultFlipU", "TextureMultFilpU")
        .rename("TextureMultFlipV", "TextureMultFilpV")
        .id("vfx-texture-mult")
        .title("An emitter's second texture and its fields, in textureMult")
        .at_build(Version::new(13, 1))
        .record(Record::game_change(5))
}

/// Every built-in table rule.
pub fn all() -> Vec<Rule> {
    vec![
        filtering(),
        linger(),
        flex_shape(),
        legacy_simple(),
        texture_mult(),
    ]
}
