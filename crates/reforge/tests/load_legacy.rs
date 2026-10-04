//! `legacy-formats`: a `.dds` or `.sco` at a path the game dropped, made the `.tex` or `.scb`
//! the game reads at the same stem, over a mod and a game built here.

use std::collections::BTreeMap;

use fs_err as fs;
use reforge::archive::file_kind;
use reforge::diag::{Channel, Diag, Diagnostics, Render, Stage};
use reforge::game::{NameBook, Names};
use reforge::history::NoHistory;
use reforge::load::files::{FileFixCx, FileFixup, one_record};
use reforge::load::legacy::{
    CODES, Conversion, ConvertError, FORMATS, FormatsConverted, FormatsUnreadable, LegacyFormats,
    sco_to_scb, twin,
};
use reforge::model::entity::{
    FileId, FileKind, FileNode, FileSource, FileUse, MadeBy, ModMeta, ModWad, WadId, WadLayout,
    WadRole,
};
use reforge::model::{ChunkPath, ContentId, Mod, NoNames, Spelled, WadName, path_hash};
use reforge_testkit::MemoryGame;

/// A texture the game ships as `.tex`.
const IDLE: &str = "assets/characters/test/skins/base/particles/idle";
/// A static mesh the game ships as `.scb`.
const BOMB: &str = "assets/characters/test/skins/base/particles/bomb";

/// A DDS by its magic: the fixup takes its bytes as they are and reads no further.
fn dds() -> Vec<u8> {
    let mut out = b"DDS ".to_vec();
    out.extend((0..120u8).map(|i| i.wrapping_mul(37)));
    out
}

const SCO: &str = "[ObjectBegin]
Name= bomb
CentralPoint= 0.5 0.5 0.0
Verts= 3
0.0 0.0 0.0
1.0 0.0 0.0
0.0 1.0 0.0
Faces= 1
3 0 1 2 bomb 0.0 0.0 1.0 0.0 0.0 1.0
[ObjectEnd]
";

/// A mod, the game it is judged against, and the run's names and log.
struct Fixture {
    module: Mod,
    wad: WadId,
    game: MemoryGame,
    book: NameBook,
    log: Diagnostics,
}

impl Fixture {
    /// A mod of one empty champion WAD, and a game shipping the texture and the mesh.
    fn new() -> Self {
        let mut module = Mod::new(ModMeta {
            file_name: Spelled::new("legacy.fantome"),
            sha256: ContentId::of(b"legacy"),
            mod_id: "000000000000".into(),
            name: None,
            author: None,
            version: None,
            description: None,
        });
        let wad = module.wads.push(wad("Test.wad.client"));
        let mut game = MemoryGame::new("Test.wad.client", "16.19.1");
        game.insert(&format!("{IDLE}.tex"), b"the game's idle".to_vec());
        game.insert(&format!("{BOMB}.scb"), b"the game's bomb".to_vec());
        Self {
            module,
            wad,
            game,
            book: NameBook::new(),
            log: Diagnostics::new(),
        }
    }

    /// Adds a WAD to the mod.
    fn wad(&mut self, name: &str) -> WadId {
        self.module.wads.push(wad(name))
    }

    /// Adds a file the archive ships, of the kind its bytes are.
    fn file_in(&mut self, wad: WadId, path: &str, bytes: &[u8]) -> FileId {
        let path = ChunkPath::parse(path).unwrap();
        self.module
            .push_file(FileNode {
                wad,
                hash: path.hash(),
                path: Some(path),
                size: bytes.len() as u64,
                content: ContentId::of(bytes),
                kind: file_kind(bytes),
                bytes: bytes.into(),
                used: FileUse::Unused,
                source: FileSource::Archive,
            })
            .expect("the path is free")
    }

    fn file(&mut self, path: &str, bytes: &[u8]) -> FileId {
        self.file_in(self.wad, path, bytes)
    }

    /// Runs the fixup as `load` runs it.
    fn run(&mut self) {
        let fixup = LegacyFormats;
        let mut stage = self.log.at(Stage::Load);
        let mut sink = stage.by(fixup.id(), one_record(&fixup));
        fixup
            .fix(&mut FileFixCx {
                module: &mut self.module,
                game: &self.game,
                history: &NoHistory,
                names: &Names::default(),
                book: &mut self.book,
                sink: &mut sink,
            })
            .expect("the fixup asks the history nothing");
    }

    /// The file of the first WAD at a path.
    fn at(&self, path: &str) -> Option<(FileId, &FileNode)> {
        let id = self.module.file_at(self.wad, path_hash(path))?;
        Some((id, &self.module.files[id]))
    }

    fn codes(&self) -> Vec<&'static str> {
        self.log.iter().map(|d| d.code.name).collect()
    }

    /// The message of every diagnostic, with the mod's files spelled.
    fn messages(&self) -> Vec<String> {
        let cx = Render {
            names: &NoNames,
            module: Some(&self.module),
        };
        self.log.iter().map(|d| d.message(&cx)).collect()
    }
}

fn wad(name: &str) -> ModWad {
    let name = WadName::parse(name).unwrap();
    ModWad {
        role: WadRole::Champion {
            champion: name.stem().to_string(),
        },
        name,
        layout: WadLayout::Loose,
        files: Vec::new(),
        packed_locale: None,
    }
}

/// The one finding of a type.
fn one<T: Diag>(log: &Diagnostics) -> &T {
    let mut found = log.of::<T>();
    let first = found
        .next()
        .unwrap_or_else(|| panic!("no {} was raised", T::CODE));
    assert_eq!(found.count(), 0, "{} was raised more than once", T::CODE);
    first
}

#[test]
fn the_twin_of_a_path_is_the_same_stem_in_the_format_the_game_reads() {
    let twin_of = |path: &str| {
        let (to, format) = twin(&ChunkPath::parse(path).unwrap())?;
        Some((to.as_str().to_string(), format.dropped, format.kind))
    };
    assert_eq!(
        twin_of("ASSETS/Characters/Test/Skins/Base/Test_TX_CM.DDS"),
        Some((
            "assets/characters/test/skins/base/test_tx_cm.tex".into(),
            "dds",
            FileKind::Tex
        ))
    );
    assert_eq!(
        twin_of("assets/shared/particles/bomb.sco"),
        Some((
            "assets/shared/particles/bomb.scb".into(),
            "sco",
            FileKind::Scb
        ))
    );
    assert_eq!(twin_of("assets/shared/particles/bomb.scb"), None);
    assert_eq!(twin_of("assets/shared/particles/idle.tex"), None);
    assert_eq!(twin_of("assets/shared/particles/dds"), None);
    let records: Vec<String> = FORMATS.iter().map(|f| f.record.to_string()).collect();
    assert_eq!(records, ["#GAME-CHANGE-0007", "#GAME-CHANGE-0008"]);
}

#[test]
fn an_sco_becomes_an_scb() {
    let out = sco_to_scb(SCO.as_bytes()).unwrap();
    let mesh = ltk_mesh::StaticMesh::from_reader(&mut out.as_slice()).unwrap();
    assert_eq!((mesh.vertices().len(), mesh.faces().len()), (3, 1));
    assert!(matches!(
        sco_to_scb(b"not a mesh"),
        Err(ConvertError::Read { .. })
    ));
}

#[test]
fn an_sco_declaring_more_than_its_bytes_can_list_is_refused_before_it_is_read() {
    // The reader reserves memory for a declared count before it reads one line of it.
    let vertices = SCO.replace("Verts= 3", "Verts= 4000000000000");
    assert_eq!(
        sco_to_scb(vertices.as_bytes()),
        Err(ConvertError::CountLargerThanData {
            what: "vertices",
            declared: 4_000_000_000_000,
            bytes: vertices.len(),
            fit: vertices.len() / 6,
        })
    );
    let faces = SCO.replace("Faces= 1", "Faces= 4000000000000");
    assert!(matches!(
        sco_to_scb(faces.as_bytes()),
        Err(ConvertError::CountLargerThanData { what: "faces", .. })
    ));
    // The counts are found after the two optional lines and the vertex colors too.
    let colored = "[ObjectBegin]\nName= x\nCentralPoint= 0 0 0\nPivotPoint= 0 0 0\n\
        VertexColors= 1\nVerts= 1\n0 0 0\n255 255 255 255\nFaces= 77777777777\n";
    assert!(matches!(
        sco_to_scb(colored.as_bytes()),
        Err(ConvertError::CountLargerThanData {
            what: "faces",
            declared: 77_777_777_777,
            ..
        })
    ));
    let pivot = "[ObjectBegin]\nName= x\nCentralPoint= 0 0 0\nPivotPoint= 0 0 0\n\
        Verts= 99999999999\n";
    assert!(matches!(
        sco_to_scb(pivot.as_bytes()),
        Err(ConvertError::CountLargerThanData {
            what: "vertices",
            ..
        })
    ));
    // A count the bytes can list is the reader's to judge.
    let short = SCO.replace("Verts= 3", "Verts= 5");
    assert!(matches!(
        sco_to_scb(short.as_bytes()),
        Err(ConvertError::Read { .. })
    ));
}

#[test]
fn legacy_files_are_made_the_twins_the_game_reads() {
    // An older mod: its texture as `.dds`, its mesh as `.sco`, where the game ships `.tex` and
    // `.scb` at the same stems.
    let mut f = Fixture::new();
    let texture = f.file(&format!("{IDLE}.dds"), &dds());
    let mesh = f.file(&format!("{BOMB}.sco"), SCO.as_bytes());
    f.run();

    let (tex_id, tex) = f.at(&format!("{IDLE}.tex")).expect("the texture is made");
    let (scb_id, scb) = f.at(&format!("{BOMB}.scb")).expect("the mesh is made");
    // #GAME-BEHAVIOR-0003: the texture's bytes as they are, a DDS at a `.tex` path.
    assert_eq!((tex.kind, scb.kind), (FileKind::Dds, FileKind::Scb));
    assert_eq!(&*tex.bytes, dds().as_slice());
    assert_eq!(tex.content, ContentId::of(&dds()));
    assert_eq!(tex.size, dds().len() as u64);
    let read = ltk_mesh::StaticMesh::from_reader(&mut &*scb.bytes).unwrap();
    assert_eq!((read.vertices().len(), read.faces().len()), (3, 1));
    assert_eq!(scb.content, ContentId::of(&scb.bytes));
    assert_eq!(scb.size, scb.bytes.len() as u64);
    let made = |from| FileSource::Made {
        from,
        by: MadeBy::LegacyFormat,
    };
    assert_eq!((tex.source, scb.source), (made(texture), made(mesh)));
    assert_eq!((tex.used, scb.used), (FileUse::Unused, FileUse::Unused));
    assert_eq!(tex.path.as_ref().unwrap().as_str(), format!("{IDLE}.tex"));
    // Each is a file of the WAD its original is in, and the original stays.
    assert_eq!(f.module.wads[f.wad].files, [texture, mesh, tex_id, scb_id]);
    assert_eq!(f.module.files[texture].source, FileSource::Archive);
    // The run's names learn the paths made.
    assert_eq!(
        f.book.path(path_hash(&format!("{BOMB}.scb"))),
        Some(format!("{BOMB}.scb").as_str())
    );

    assert_eq!(f.codes(), ["file.format-converted"]);
    let found = one::<FormatsConverted>(&f.log);
    assert_eq!(found.wad, f.wad);
    let items: Vec<_> = found
        .files
        .iter()
        .map(|c| (c.from, c.made, c.how, c.record.to_string()))
        .collect();
    assert_eq!(
        items,
        [
            (
                texture,
                tex_id,
                Conversion::DdsAsTex,
                "#GAME-CHANGE-0007".to_string()
            ),
            (
                mesh,
                scb_id,
                Conversion::ScoAsScb,
                "#GAME-CHANGE-0008".to_string()
            ),
        ]
    );
    assert_eq!(
        f.messages(),
        [
            "2 files at paths the game dropped, made the file it reads at the same stem: \
          1 .dds as .tex, unchanged; 1 .sco as .scb"
        ]
    );
    let diagnostic = f.log.iter().next().unwrap();
    assert_eq!(diagnostic.by.as_deref(), Some("legacy-formats"));
    assert_eq!(
        diagnostic.data()["files"][0],
        serde_json::json!({
            "from": "%f0", "made": "%f2", "how": "dds-as-tex", "record": "#GAME-CHANGE-0007"
        })
    );
}

#[test]
fn a_second_run_makes_nothing() {
    let mut f = Fixture::new();
    f.file(&format!("{IDLE}.dds"), &dds());
    f.file(&format!("{BOMB}.sco"), SCO.as_bytes());
    f.run();
    let (files, diagnostics) = (f.module.files.len(), f.log.len());
    assert_eq!((files, diagnostics), (4, 1));
    f.run();
    assert_eq!((f.module.files.len(), f.log.len()), (files, diagnostics));
}

#[test]
fn the_mods_own_twin_is_left_alone() {
    // The mod ships the `.tex` too: that one is the mod's texture, and nothing is made.
    let mut f = Fixture::new();
    f.file(&format!("{IDLE}.dds"), &dds());
    let own = f.file(&format!("{IDLE}.tex"), b"the mod's own tex");
    f.run();
    assert!(f.log.is_empty(), "{:?}", f.codes());
    assert_eq!(f.module.files.len(), 2);
    assert_eq!(f.at(&format!("{IDLE}.tex")).map(|(id, _)| id), Some(own));
}

#[test]
fn a_file_the_game_still_ships_is_not_converted() {
    // A `.dds` at a path the game has is the game's to name.
    let mut f = Fixture::new();
    f.game
        .insert(&format!("{IDLE}.dds"), b"the game's old dds".to_vec());
    f.file(&format!("{IDLE}.dds"), &dds());
    f.run();
    assert!(f.log.is_empty(), "{:?}", f.codes());
    assert_eq!(f.module.files.len(), 1);
}

#[test]
fn a_twin_the_game_does_not_ship_is_not_made() {
    let mut f = Fixture::new();
    f.file(
        "assets/characters/test/skins/base/particles/own.dds",
        &dds(),
    );
    f.file(
        "assets/characters/test/skins/base/particles/own.sco",
        SCO.as_bytes(),
    );
    f.run();
    assert!(f.log.is_empty(), "{:?}", f.codes());
    assert_eq!(f.module.files.len(), 2);
}

#[test]
fn a_file_already_in_the_format_the_game_reads_is_copied() {
    // A TEX and an SCB saved under the dropped extensions.
    let mut f = Fixture::new();
    let tex_bytes = b"TEX\0 a texture in the game's container";
    let scb_bytes = sco_to_scb(SCO.as_bytes()).unwrap();
    let texture = f.file(&format!("{IDLE}.dds"), tex_bytes);
    let mesh = f.file(&format!("{BOMB}.sco"), &scb_bytes);
    assert_eq!(
        (f.module.files[texture].kind, f.module.files[mesh].kind),
        (FileKind::Tex, FileKind::Scb)
    );
    f.run();
    let (_, tex) = f.at(&format!("{IDLE}.tex")).expect("the texture is made");
    let (_, scb) = f.at(&format!("{BOMB}.scb")).expect("the mesh is made");
    assert_eq!((tex.kind, scb.kind), (FileKind::Tex, FileKind::Scb));
    assert_eq!((&*tex.bytes, &*scb.bytes), (&tex_bytes[..], &scb_bytes[..]));
    let found = one::<FormatsConverted>(&f.log);
    let how: Vec<Conversion> = found.files.iter().map(|c| c.how).collect();
    assert_eq!(how, [Conversion::AsIs, Conversion::AsIs]);
    assert_eq!(
        f.messages(),
        [
            "2 files at paths the game dropped, made the file it reads at the same stem: \
          2 already in the format the game reads, copied"
        ]
    );
}

#[test]
fn bytes_of_another_kind_than_the_twin_holds_are_left() {
    // A DDS named `.sco`, an SCO named `.dds`, and a file that is neither a texture nor a mesh.
    let mut f = Fixture::new();
    f.file(&format!("{BOMB}.sco"), &dds());
    f.file(&format!("{IDLE}.dds"), SCO.as_bytes());
    f.game.insert(
        "assets/characters/test/skins/base/readme.tex",
        b"the game's".to_vec(),
    );
    f.file("assets/characters/test/skins/base/readme.dds", b"a note");
    f.run();
    assert!(f.log.is_empty(), "{:?}", f.codes());
    assert_eq!(f.module.files.len(), 3);
}

#[test]
fn an_sco_that_does_not_read_stays_and_is_reported() {
    let mut f = Fixture::new();
    // The line the reader refuses holds an escape sequence, which the reason repeats.
    let broken = "[ObjectBegin]\nName= bomb\nCentralPoint= 0 0 0\nVerts=\u{1b}[31m many\n";
    let mesh = f.file(&format!("{BOMB}.sco"), broken.as_bytes());
    assert_eq!(f.module.files[mesh].kind, FileKind::Sco);
    f.file(&format!("{IDLE}.dds"), &dds());
    f.run();

    assert!(f.at(&format!("{BOMB}.scb")).is_none());
    assert!(f.at(&format!("{IDLE}.tex")).is_some());
    assert_eq!(
        f.codes(),
        ["file.format-converted", "file.format-unreadable"]
    );
    let found = one::<FormatsUnreadable>(&f.log);
    assert_eq!(found.wad, f.wad);
    assert_eq!(found.files.len(), 1);
    assert_eq!(found.files[0].file, mesh);
    assert!(matches!(found.files[0].why, ConvertError::Read { .. }));
    let message = &f.messages()[1];
    assert!(
        message.starts_with(&format!(
            "1 file at paths the game dropped that do not read, left where they are: \
             {BOMB}.sco (not a readable SCO: "
        )),
        "{message}"
    );
    assert!(!message.contains('\u{1b}'), "{message:?}");
    let data = f.log.iter().nth(1).unwrap().data();
    assert_eq!(data["files"][0]["file"], "%f0");
    assert_eq!(data["files"][0]["why"], "read");
}

#[test]
fn an_sco_the_mesh_writer_panics_on_stays_and_is_reported() {
    // `ltk_io_ext` 0.4.7 asserts, in a build with debug assertions, that a mesh's name fits the
    // 128 bytes an SCB keeps for it. A build without them cuts the name short and writes.
    let name = "b".repeat(129);
    let long = SCO.replace("Name= bomb", &format!("Name= {name}"));
    let made = sco_to_scb(long.as_bytes());
    if !cfg!(debug_assertions) {
        assert!(made.is_ok(), "{made:?}");
        return;
    }
    let Err(ConvertError::Panicked { reason }) = &made else {
        panic!("the panic was not caught as an error: {made:?}");
    };
    assert!(reason.contains("str.len() <= N"), "{reason}");

    let mut f = Fixture::new();
    let mesh = f.file(&format!("{BOMB}.sco"), long.as_bytes());
    f.run();
    assert!(f.at(&format!("{BOMB}.scb")).is_none());
    assert_eq!(f.codes(), ["file.format-unreadable"]);
    let found = one::<FormatsUnreadable>(&f.log);
    assert_eq!(found.files[0].file, mesh);
    assert_eq!(found.files[0].why, made.unwrap_err());
    let data = f.log.iter().next().unwrap().data();
    assert_eq!(data["files"][0]["why"], "panicked");
}

#[test]
fn each_wad_has_its_own_finding_and_its_own_twin() {
    let mut f = Fixture::new();
    let other = f.wad("Other.wad.client");
    let second = f.file_in(other, &format!("{IDLE}.dds"), &dds());
    let first = f.file(&format!("{IDLE}.dds"), &dds());
    f.run();
    let wads: Vec<(WadId, Vec<FileId>)> = f
        .log
        .of::<FormatsConverted>()
        .map(|found| (found.wad, found.files.iter().map(|c| c.from).collect()))
        .collect();
    assert_eq!(wads, [(f.wad, vec![first]), (other, vec![second])]);
    let twin = path_hash(&format!("{IDLE}.tex"));
    let made = |wad| f.module.files[f.module.file_at(wad, twin).unwrap()].source;
    assert_eq!(
        (made(f.wad), made(other)),
        (
            FileSource::Made {
                from: first,
                by: MadeBy::LegacyFormat
            },
            FileSource::Made {
                from: second,
                by: MadeBy::LegacyFormat
            }
        )
    );
}

#[test]
fn a_file_the_archive_names_by_hash_is_converted_when_the_names_spell_it() {
    let mut f = Fixture::new();
    let named = f.file(&format!("{IDLE}.dds"), &dds());
    f.module.files[named].path = None;
    let unnamed = f.file(&format!("{BOMB}.sco"), SCO.as_bytes());
    f.module.files[unnamed].path = None;
    f.book.learn_path(&format!("{IDLE}.dds"));
    f.run();
    assert!(f.at(&format!("{IDLE}.tex")).is_some());
    assert!(f.at(&format!("{BOMB}.scb")).is_none());
    assert_eq!(one::<FormatsConverted>(&f.log).files.len(), 1);
}

#[test]
fn the_codes_are_the_ones_the_codes_table_lists() {
    let file = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/design/CODES.md");
    let text = fs::read_to_string(file).unwrap();
    let documented: BTreeMap<&str, &str> = text
        .lines()
        .filter_map(|line| {
            let mut cells = line.strip_prefix("| `")?.split('|');
            let code = cells.next()?.trim().trim_end_matches('`');
            Some((code, cells.next()?.trim()))
        })
        .collect();
    assert_eq!(CODES, [FormatsConverted::CODE, FormatsUnreadable::CODE]);
    for code in CODES {
        assert_eq!(
            documented.get(code.name).copied(),
            Some(code.severity.as_str()),
            "{code}"
        );
        assert_eq!(
            code.channel,
            Channel::Output,
            "{code}: what a file fixup made"
        );
    }
}
