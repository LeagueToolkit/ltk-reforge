//! A synthetic game for tests of the stages: one champion, `test`, with four skins, and a
//! sub-character, `testpet`, with a base skin of its own.
//!
//! | slot | what | mesh, skeleton | graph | resolver |
//! |---|---|---|---|---|
//! | 0 | base | the base's | `Animations/Skin0` (base clips) | `Skin0/Resources` |
//! | 3 | premium | its own | `Animations/Skin3` (its own clips) | `Skin3/Resources` |
//! | 7 | premium | the base's | `Animations/Skin0` | `Skin7/Resources` |
//! | 9 | chroma of 3 | skin 3's | `Animations/Skin3` | `Skin3/Resources` |

use crate::bins::{Skin, bin, blob, entry, graph, particle, resolver};
use crate::game::MemoryGame;

/// The WAD of the champion.
pub const WAD: &str = "Test.wad.client";
/// The base mesh.
pub const BASE_MESH: &str = "assets/characters/test/skins/base/test.skn";
/// The base skeleton.
pub const BASE_SKEL: &str = "assets/characters/test/skins/base/test.skl";
/// The base texture.
pub const BASE_TEX: &str = "assets/characters/test/skins/base/test.tex";
/// The texture the idle particle of the base names.
pub const BASE_PARTICLE_TEX: &str = "assets/characters/test/skins/base/particles/idle.tex";
/// The mesh of skin 3.
pub const S3_MESH: &str = "assets/characters/test/skins/skin03/test_skin03.skn";
/// The skeleton of skin 3.
pub const S3_SKEL: &str = "assets/characters/test/skins/skin03/test_skin03.skl";
/// The texture of skin 3.
pub const S3_TEX: &str = "assets/characters/test/skins/skin03/test_skin03.tex";
/// The clips of the base.
pub const BASE_CLIPS: &[&str] = &[
    "assets/characters/test/skins/base/animations/idle.anm",
    "assets/characters/test/skins/base/animations/run.anm",
];
/// The clips of skin 3.
pub const S3_CLIPS: &[&str] = &[
    "assets/characters/test/skins/skin03/animations/idle.anm",
    "assets/characters/test/skins/skin03/animations/recall.anm",
];
/// An effects texture of skin 3, which no object of the game names.
pub const S3_PARTICLE_TEX: &str =
    "assets/characters/test/skins/skin03/particles/test_skin03_glow.dds";
/// The particle the base skin bin of the game holds besides its entry, graph and resolver.
pub const BASE_PARTICLE: &str = "Characters/Test/Skins/Skin0/Particles/Test_Base_Idle";
/// The mesh of the sub-character.
pub const PET_MESH: &str = "assets/characters/testpet/skins/base/testpet.skn";
/// The skeleton of the sub-character.
pub const PET_SKEL: &str = "assets/characters/testpet/skins/base/testpet.skl";

/// The path of the skin bin of a character at a slot.
pub fn skin_bin(character: &str, slot: u16) -> String {
    format!("data/characters/{character}/skins/skin{slot}.bin")
}

/// The entry of the base skin.
pub fn base_skin() -> Skin {
    Skin {
        mesh: BASE_MESH.into(),
        skeleton: BASE_SKEL.into(),
        texture: BASE_TEX.into(),
        graph: "Characters/Test/Animations/Skin0".into(),
        resolver: "Characters/Test/Skins/Skin0/Resources".into(),
        name: "Test".into(),
        class: None,
        parent: None,
    }
}

/// The entry of skin 3.
pub fn skin3() -> Skin {
    Skin {
        mesh: S3_MESH.into(),
        skeleton: S3_SKEL.into(),
        texture: S3_TEX.into(),
        graph: "Characters/Test/Animations/Skin3".into(),
        resolver: "Characters/Test/Skins/Skin3/Resources".into(),
        name: "TestSkin03".into(),
        class: Some(1),
        parent: None,
    }
}

/// The entry of skin 7.
pub fn skin7() -> Skin {
    Skin {
        resolver: "Characters/Test/Skins/Skin7/Resources".into(),
        name: "TestSkin07".into(),
        class: Some(1),
        ..base_skin()
    }
}

/// The entry of skin 9: a chroma of skin 3.
pub fn skin9() -> Skin {
    Skin {
        name: "TestSkin09".into(),
        class: Some(2),
        parent: Some(3),
        ..skin3()
    }
}

/// The entry of the base skin of the sub-character.
pub fn pet_skin() -> Skin {
    Skin {
        mesh: PET_MESH.into(),
        skeleton: PET_SKEL.into(),
        texture: "assets/characters/testpet/skins/base/testpet.tex".into(),
        graph: "Characters/TestPet/Animations/Skin0".into(),
        resolver: "Characters/TestPet/Skins/Skin0/Resources".into(),
        name: "TestPet".into(),
        class: None,
        parent: None,
    }
}

/// The skin bin the game holds for the sub-character.
pub fn pet_bin() -> Vec<u8> {
    bin(vec![
        entry("Characters/TestPet/Skins/Skin0", &pet_skin()),
        graph(
            "Characters/TestPet/Animations/Skin0",
            &["assets/characters/testpet/skins/base/animations/idle.anm"],
        ),
    ])
}

/// The game, at build 16.19.1.
pub fn game() -> MemoryGame {
    game_at("16.19.1")
}

/// The game, at a build.
pub fn game_at(build: &str) -> MemoryGame {
    let mut game = MemoryGame::new(WAD, build);
    game.insert(
        &skin_bin("test", 0),
        bin(vec![
            entry("Characters/Test/Skins/Skin0", &base_skin()),
            graph("Characters/Test/Animations/Skin0", BASE_CLIPS),
            resolver("Characters/Test/Skins/Skin0/Resources"),
            particle(BASE_PARTICLE, BASE_PARTICLE_TEX),
        ]),
    );
    game.insert(
        &skin_bin("test", 3),
        bin(vec![
            entry("Characters/Test/Skins/Skin3", &skin3()),
            graph("Characters/Test/Animations/Skin3", S3_CLIPS),
            resolver("Characters/Test/Skins/Skin3/Resources"),
        ]),
    );
    game.insert(
        &skin_bin("test", 7),
        bin(vec![
            entry("Characters/Test/Skins/Skin7", &skin7()),
            resolver("Characters/Test/Skins/Skin7/Resources"),
        ]),
    );
    game.insert(
        &skin_bin("test", 9),
        bin(vec![entry("Characters/Test/Skins/Skin9", &skin9())]),
    );
    game.insert(&skin_bin("testpet", 0), pet_bin());
    let files = [
        BASE_PARTICLE_TEX,
        BASE_MESH,
        BASE_SKEL,
        BASE_TEX,
        S3_MESH,
        S3_SKEL,
        S3_TEX,
        S3_PARTICLE_TEX,
        PET_MESH,
        PET_SKEL,
    ];
    for path in files.iter().chain(BASE_CLIPS).chain(S3_CLIPS) {
        game.insert(path, blob(path));
    }
    game
}
