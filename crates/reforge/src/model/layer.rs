//! The layers of a package: the parts a player turns on and off.

use serde::Serialize;

/// A part of a package a player turns on or off.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Layer {
    /// What is always on.
    Base,
    /// The mod's own voice lines, of one locale when they come from a locale WAD
    /// (#ADR-0028).
    Voice(Option<String>),
    /// Spell icons and HUD textures (#ADR-0029).
    Hud,
}

impl Layer {
    /// The layer's name in a project and a package: `base`, `voice`, `voice-en-us`, `hud`.
    pub fn name(&self) -> String {
        match self {
            Layer::Base => "base".to_string(),
            Layer::Voice(None) => "voice".to_string(),
            Layer::Voice(Some(locale)) => format!("voice-{}", locale.replace('_', "-")),
            Layer::Hud => "hud".to_string(),
        }
    }

    /// The layer's name for a person.
    pub fn title(&self) -> String {
        match self {
            Layer::Base => "Base".to_string(),
            Layer::Voice(None) => "Voice".to_string(),
            Layer::Voice(Some(locale)) => format!("Voice ({locale})"),
            Layer::Hud => "Spell icons and HUD".to_string(),
        }
    }

    /// One line on what the layer holds.
    pub fn description(&self) -> String {
        match self {
            Layer::Base => "Base layer of the mod".to_string(),
            Layer::Voice(None) => "The mod's own voice lines".to_string(),
            Layer::Voice(Some(locale)) => {
                format!("The mod's own voice lines, from its {locale} WAD")
            }
            Layer::Hud => "The mod's spell icons and HUD textures; they show for the champion \
                           under every skin"
                .to_string(),
        }
    }
}
