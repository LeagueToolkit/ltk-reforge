//! The class and field names a run reads by name. A name is hashed where it is used, so one
//! spelled wrong reads nothing: each is written once, here.

/// Class names.
pub mod class {
    /// A skin entry.
    pub const SKIN_CHARACTER_DATA_PROPERTIES: &str = "SkinCharacterDataProperties";
    /// A resolver.
    pub const RESOURCE_RESOLVER: &str = "ResourceResolver";
    /// A graph.
    pub const ANIMATION_GRAPH_DATA: &str = "AnimationGraphData";
    /// A clip of a graph that plays one animation file.
    pub const ATOMIC_CLIP_DATA: &str = "AtomicClipData";
    /// A graph event that shows and hides submeshes.
    pub const SUBMESH_VISIBILITY_EVENT_DATA: &str = "SubmeshVisibilityEventData";
    /// A material.
    pub const STATIC_MATERIAL: &str = "StaticMaterialDef";

    /// Every class name above.
    pub const ALL: &[&str] = &[
        SKIN_CHARACTER_DATA_PROPERTIES,
        RESOURCE_RESOLVER,
        ANIMATION_GRAPH_DATA,
        ATOMIC_CLIP_DATA,
        SUBMESH_VISIBILITY_EVENT_DATA,
        STATIC_MATERIAL,
    ];
}

/// Field names.
pub mod field {
    /// A skin entry's mesh block.
    pub const SKIN_MESH_PROPERTIES: &str = "skinMeshProperties";
    /// The mesh file of a mesh block.
    pub const SIMPLE_SKIN: &str = "simpleSkin";
    /// The skeleton file of a mesh block.
    pub const SKELETON: &str = "skeleton";
    /// The texture file of a mesh block.
    pub const TEXTURE: &str = "texture";
    /// The submeshes a mesh block hides at first.
    pub const INITIAL_SUBMESH_TO_HIDE: &str = "initialSubmeshToHide";
    /// A skin entry's animation block.
    pub const SKIN_ANIMATION_PROPERTIES: &str = "skinAnimationProperties";
    /// The graph an animation block names.
    pub const ANIMATION_GRAPH_DATA: &str = "animationGraphData";
    /// The resolver a skin entry names.
    pub const RESOURCE_RESOLVER: &str = "mResourceResolver";
    /// The skin's name, `LuxSkin07`.
    pub const CHAMPION_SKIN_NAME: &str = "championSkinName";
    /// A skin entry's audio block.
    pub const SKIN_AUDIO_PROPERTIES: &str = "skinAudioProperties";
    /// The tags of an audio block.
    pub const TAG_EVENT_LIST: &str = "tagEventList";
    /// The bank units of an audio block.
    pub const BANK_UNITS: &str = "bankUnits";
    /// A bank unit's name.
    pub const BANK_UNIT_NAME: &str = "name";
    /// A bank unit's bank paths.
    pub const BANK_PATH: &str = "bankPath";
    /// A bank unit's events.
    pub const BANK_UNIT_EVENTS: &str = "events";
    /// Whether a bank unit is voice.
    pub const VOICE_OVER: &str = "voiceOver";
    /// What a skin is: the base, a premium skin, a chroma.
    pub const SKIN_CLASSIFICATION: &str = "skinClassification";
    /// The slot of the skin a chroma recolors.
    pub const SKIN_PARENT: &str = "skinParent";
    /// A resolver's map of effect keys to effects.
    pub const RESOURCE_MAP: &str = "resourceMap";
    /// A material's name: the hash of its entry name (#GAME-BEHAVIOR-0030).
    pub const NAME: &str = "name";
    /// A graph's clips.
    pub const CLIP_DATA_MAP: &str = "mClipDataMap";
    /// A clip's events.
    pub const EVENT_DATA_MAP: &str = "mEventDataMap";
    /// The animation file block of an atomic clip.
    pub const ANIMATION_RESOURCE_DATA: &str = "mAnimationResourceData";
    /// The animation file of an atomic clip.
    pub const ANIMATION_FILE_PATH: &str = "mAnimationFilePath";
    /// The submeshes a visibility event shows.
    pub const SHOW_SUBMESH_LIST: &str = "mShowSubmeshList";
    /// The characters a skin entry loads with its own, by name.
    pub const EXTRA_CHARACTER_PRELOADS: &str = "extraCharacterPreloads";

    /// Every field name above. A run learns them, so that a field path spells them whatever
    /// tables are installed.
    pub const ALL: &[&str] = &[
        SKIN_MESH_PROPERTIES,
        SIMPLE_SKIN,
        SKELETON,
        TEXTURE,
        INITIAL_SUBMESH_TO_HIDE,
        SKIN_ANIMATION_PROPERTIES,
        ANIMATION_GRAPH_DATA,
        RESOURCE_RESOLVER,
        CHAMPION_SKIN_NAME,
        SKIN_AUDIO_PROPERTIES,
        TAG_EVENT_LIST,
        BANK_UNITS,
        BANK_UNIT_NAME,
        BANK_PATH,
        BANK_UNIT_EVENTS,
        VOICE_OVER,
        SKIN_CLASSIFICATION,
        SKIN_PARENT,
        RESOURCE_MAP,
        CLIP_DATA_MAP,
        EVENT_DATA_MAP,
        ANIMATION_RESOURCE_DATA,
        ANIMATION_FILE_PATH,
        SHOW_SUBMESH_LIST,
        EXTRA_CHARACTER_PRELOADS,
    ];
}
