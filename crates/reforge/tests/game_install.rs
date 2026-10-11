//! The install: a path resolves to the first WAD holding it, champion WADs first; chunks are
//! read on demand; what does not read is a typed error or a diagnostic. The fake in memory
//! answers as an install does.

use std::io::{Cursor, Write};

use camino::{Utf8Path, Utf8PathBuf};
use fs_err as fs;
use ltk_wad::{WadBuilder, WadChunkBuilder};
use reforge::diag::{Channel, Diag, Diagnostics, Input, Render, Severity, Stage, Subject};
use reforge::game::{
    Build, Game, GameError, Install, InstallError, NoBuild, SizeIndex, VersionUnknown,
    WadsUnreadable,
};
use reforge::model::{WadHash, WadName, path_hash};
use tempfile::TempDir;

use reforge_testkit::game::MemoryGame;

const VERSION: &str = "16.19.8207193+branch.releases-16-19.content.release";

const SKIN: &str = "data/characters/lux/skins/skin0.bin";
const SHARED: &str = "assets/shared/particles/glow.tex";
const SOUND: &str = "assets/sounds/wwise2016/sfx/characters/lux/skins/base/lux_base_sfx_audio.bnk";

/// The bytes of a WAD with these chunks.
fn wad(chunks: &[(&str, &[u8])]) -> Vec<u8> {
    let mut builder = WadBuilder::default();
    for (path, _) in chunks {
        builder = builder.with_chunk(WadChunkBuilder::default().with_path(path));
    }
    let mut out = Cursor::new(Vec::new());
    builder
        .build_to_writer(&mut out, |hash, chunk| {
            let (_, bytes) = chunks
                .iter()
                .find(|(path, _)| path_hash(path) == hash)
                .unwrap_or_else(|| panic!("the builder asked for chunk {hash:016x}, not given"));
            chunk.write_all(bytes)?;
            Ok(())
        })
        .unwrap();
    out.into_inner()
}

/// An install directory in a temporary directory.
struct Fixture {
    _dir: TempDir,
    root: Utf8PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::from_path_buf(dir.path().join("League of Legends"))
            .expect("the temporary directory is UTF-8");
        fs::create_dir_all(root.join("Game/DATA/FINAL/Champions")).unwrap();
        Self { _dir: dir, root }
    }

    /// An install of three WADs at a build: two champions sharing a chunk, and a WAD that sorts
    /// before both and holds a chunk of a champion's.
    fn standard() -> Self {
        let fixture = Self::new();
        fixture.version(VERSION);
        fixture.wad(
            "Champions/Lux.wad.client",
            &[(SKIN, b"lux skin0"), (SHARED, b"glow of lux")],
        );
        fixture.wad("Champions/Ahri.wad.client", &[(SHARED, b"glow of ahri")]);
        fixture.wad(
            "Audio.wad.client",
            &[(SKIN, b"not the champion's"), (SOUND, b"a bank")],
        );
        fixture
    }

    fn data(&self) -> Utf8PathBuf {
        self.root.join("Game/DATA/FINAL")
    }

    fn file(&self, under_data: &str, bytes: &[u8]) {
        let path = self.data().join(under_data);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }

    fn wad(&self, under_data: &str, chunks: &[(&str, &[u8])]) {
        self.file(under_data, &wad(chunks));
    }

    fn version(&self, version: &str) {
        self.metadata(&format!(r#"{{ "version": "{version}" }}"#));
    }

    fn metadata(&self, json: &str) {
        fs::write(self.root.join("Game/content-metadata.json"), json).unwrap();
    }

    fn open(&self) -> Install {
        Install::open(&self.root).unwrap()
    }
}

fn names(wads: &[WadName]) -> Vec<&str> {
    wads.iter().map(WadName::as_str).collect()
}

fn raised(install: &Install) -> Diagnostics {
    let mut log = Diagnostics::new();
    install.report(&mut log.at(Stage::Read));
    log
}

#[test]
fn a_path_resolves_to_the_first_wad_holding_it_and_every_wad_is_kept() {
    let fixture = Fixture::standard();
    let install = fixture.open();
    let shared = path_hash(SHARED);
    assert!(install.contains(shared));
    assert_eq!(install.size(shared), Some(12));
    assert_eq!(install.read(shared).unwrap().unwrap(), b"glow of ahri");
    assert_eq!(
        names(&install.wads(shared)),
        ["Ahri.wad.client", "Lux.wad.client"]
    );

    let absent = path_hash("assets/none.tex");
    assert!(!install.contains(absent));
    assert_eq!(install.size(absent), None);
    assert_eq!(install.read(absent).unwrap(), None);
    assert!(install.wads(absent).is_empty());
}

#[test]
fn a_champion_wad_comes_before_a_wad_that_sorts_before_it() {
    let fixture = Fixture::standard();
    let install = fixture.open();
    let skin = path_hash(SKIN);
    assert_eq!(install.read(skin).unwrap().unwrap(), b"lux skin0");
    assert_eq!(install.size(skin), Some(9));
    assert_eq!(
        names(&install.wads(skin)),
        ["Lux.wad.client", "Audio.wad.client"]
    );
    assert_eq!(names(&install.wads(path_hash(SOUND))), ["Audio.wad.client"]);
}

#[test]
fn the_build_is_read_without_its_branch() {
    let fixture = Fixture::standard();
    let install = fixture.open();
    let build = install.build().unwrap();
    assert_eq!(build.as_str(), "16.19.8207193");
    assert_eq!(build.number(), 8_207_193);
    assert_eq!(install.no_build(), None);
}

#[test]
fn a_build_is_parts_of_digits() {
    let build = Build::parse("16.19.8207193").unwrap();
    assert_eq!(build.to_string(), "16.19.8207193");
    assert_eq!(serde_json::to_value(&build).unwrap(), "16.19.8207193");
    assert_eq!(Build::parse("16.19.8207193+branch.x"), Some(build));
    assert_eq!(Build::parse("live"), None);
    let second = Build::parse("14.8.5756478__2").unwrap();
    assert_eq!(second.as_str(), "14.8.5756478__2");
    assert_eq!(second.number(), 5756478);
    assert_eq!(second.parts().collect::<Vec<_>>(), [14, 8, 5756478]);
    assert_ne!(Some(second), Build::parse("14.8.5756478"));
    assert_eq!(Build::parse("14.8.5756478__x"), None);
    assert_eq!(Build::parse("14.8.5756478__"), None);
    assert_eq!(Build::parse("16.19."), None);
    assert_eq!(Build::parse(""), None);
    assert_eq!(Build::parse("+branch"), None);
}

#[test]
fn the_install_directory_its_game_directory_and_its_data_directory_open_the_same() {
    let fixture = Fixture::standard();
    for root in [
        fixture.root.clone(),
        fixture.root.join("Game"),
        fixture.root.join("Game/DATA/FINAL"),
    ] {
        let install = Install::open(&root).unwrap();
        assert_eq!(install.wad_count(), 3, "{root}");
        assert_eq!(
            install.build().map(Build::as_str),
            Some("16.19.8207193"),
            "{root}"
        );
        assert_eq!(install.root(), root);
        assert_eq!(install.describe(), root.as_str());
        assert!(install.contains(path_hash(SKIN)), "{root}");
    }
}

#[test]
fn an_install_says_its_build_without_being_opened() {
    let fixture = Fixture::standard();
    for root in [fixture.root.clone(), fixture.root.join("Game")] {
        let build = Install::peek_build(&root).unwrap();
        assert_eq!(build.as_ref().map(Build::as_str), Some("16.19.8207193"));
    }
}

#[test]
fn a_folder_with_no_data_is_no_install_to_peek_at() {
    let dir = tempfile::tempdir().unwrap();
    let root = Utf8Path::from_path(dir.path()).unwrap();
    fs::write(
        root.join("content-metadata.json"),
        r#"{ "version": "16.19.1" }"#,
    )
    .unwrap();
    let error = Install::peek_build(root).unwrap_err();
    assert!(
        matches!(&error, InstallError::NoData { root: named } if named == root),
        "{error}"
    );
}

#[test]
fn a_peeked_install_with_a_malformed_version_has_no_build() {
    let fixture = Fixture::standard();
    fixture.version("live");
    assert_eq!(Install::peek_build(&fixture.root).unwrap(), None);
}

#[test]
fn a_directory_with_no_champions_is_no_install() {
    let dir = tempfile::tempdir().unwrap();
    let root = Utf8Path::from_path(dir.path()).unwrap();
    let error = Install::open(root).unwrap_err();
    assert!(
        matches!(&error, InstallError::NoData { root: named } if named == root),
        "{error}"
    );
}

#[test]
fn only_wad_client_files_are_wads() {
    let fixture = Fixture::standard();
    fixture.file("Champions/Lux.wad.client.tmp", b"a patcher's leftover");
    fixture.file("Champions/notes.txt", b"not a WAD");
    fixture.wad(
        "Maps/Shipping/MAP11.WAD.CLIENT",
        &[("data/maps/map11.bin", b"a map")],
    );
    let install = fixture.open();
    assert_eq!(install.wad_count(), 4);
    assert_eq!(
        names(&install.wads(path_hash("data/maps/map11.bin"))),
        ["MAP11.WAD.CLIENT"]
    );
    assert!(install.unreadable().is_empty());
}

#[test]
fn a_locale_wad_is_no_champion_wad() {
    let fixture = Fixture::standard();
    fixture.wad(
        "Champions/Lux.en_US.wad.client",
        &[("assets/sounds/vo/lux.bnk", b"voice")],
    );
    let install = fixture.open();
    let champions: Vec<&str> = install.champion_wads().map(WadName::as_str).collect();
    assert_eq!(champions, ["Ahri.wad.client", "Lux.wad.client"]);
}

#[test]
fn chunks_of_a_champion_wad_are_found_by_their_size() {
    let fixture = Fixture::standard();
    fixture.wad(
        "Champions/Lux.en_US.wad.client",
        &[("assets/sounds/vo/lux.bnk", b"voice")],
    );
    let install = fixture.open();
    let sizes = install.sizes();
    let lux = WadName::parse("lux.wad.client").unwrap();
    assert_eq!(sizes.same_size(&lux, 9), [path_hash(SKIN)]);
    assert_eq!(sizes.same_size(&lux, 11), [path_hash(SHARED)]);
    assert!(sizes.same_size(&lux, 10).is_empty());
    // The size is the chunk's in that WAD, also where another WAD is the one read from.
    let ahri = WadName::parse("Ahri.wad.client").unwrap();
    assert_eq!(sizes.same_size(&ahri, 12), [path_hash(SHARED)]);
    // Only champion WADs are indexed.
    let audio = WadName::parse("Audio.wad.client").unwrap();
    assert!(sizes.same_size(&audio, 6).is_empty());
    let voice = WadName::parse("Lux.en_US.wad.client").unwrap();
    assert!(sizes.same_size(&voice, 5).is_empty());
}

#[test]
fn a_size_index_keeps_the_order_chunks_were_added_in() {
    let mut sizes = SizeIndex::new();
    let lux = WadName::parse("Lux.wad.client").unwrap();
    sizes.add(&lux, WadHash(7), 100);
    sizes.add(&lux, WadHash(3), 100);
    sizes.add(&lux, WadHash(5), 200);
    assert_eq!(sizes.same_size(&lux, 100), [WadHash(7), WadHash(3)]);
    assert_eq!(sizes.same_size(&lux, 200), [WadHash(5)]);
    // A WAD is found whatever the case its name is spelled in.
    let lower = WadName::parse("lux.wad.client").unwrap();
    assert_eq!(sizes.same_size(&lower, 200), [WadHash(5)]);
}

#[test]
fn a_healthy_install_raises_nothing() {
    let fixture = Fixture::standard();
    let install = fixture.open();
    assert!(raised(&install).is_empty());
}

#[test]
fn a_wad_that_does_not_read_holds_nothing_and_is_reported_once_with_the_others() {
    let fixture = Fixture::standard();
    fixture.file("Champions/Broken.wad.client", b"not a WAD at all");
    fixture.file("Maps/Cut.wad.client", b"RW");
    let install = fixture.open();
    assert_eq!(install.wad_count(), 5);
    assert_eq!(
        install.read(path_hash(SKIN)).unwrap().unwrap(),
        b"lux skin0"
    );

    let log = raised(&install);
    let found: Vec<&WadsUnreadable> = log.of::<WadsUnreadable>().collect();
    assert_eq!(found.len(), 1);
    let wads: Vec<&str> = found[0].wads.iter().map(|w| w.wad.as_str()).collect();
    assert_eq!(wads, ["Champions/Broken.wad.client", "Maps/Cut.wad.client"]);
    assert!(found[0].wads.iter().all(|w| !w.reason.is_empty()));
    assert_eq!(install.unreadable(), found[0].wads);

    let diagnostic = log.iter().next().unwrap();
    assert_eq!(diagnostic.code.name, "install.wad-unreadable");
    assert_eq!(diagnostic.severity(), Severity::Warning);
    assert_eq!(diagnostic.channel(), Channel::Run);
    assert_eq!(diagnostic.subject, Subject::Input(Input::Install));
    assert_eq!(
        diagnostic.message(&Render::plain()),
        "2 WADs of the install with a table of contents that does not read: \
         Champions/Broken.wad.client, Maps/Cut.wad.client"
    );
    assert_eq!(log.of::<VersionUnknown>().count(), 0);
}

#[test]
fn an_install_without_its_metadata_does_not_say_its_build() {
    let fixture = Fixture::standard();
    fs::remove_file(fixture.root.join("Game/content-metadata.json")).unwrap();
    let install = fixture.open();
    assert_eq!(install.build(), None);
    assert_eq!(install.no_build(), Some(&NoBuild::MetadataMissing));

    let log = raised(&install);
    let found: Vec<&VersionUnknown> = log.of::<VersionUnknown>().collect();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].why, NoBuild::MetadataMissing);
    let diagnostic = log.iter().next().unwrap();
    assert_eq!(diagnostic.code, VersionUnknown::CODE);
    assert_eq!(diagnostic.code.name, "install.version-unknown");
    assert_eq!(diagnostic.severity(), Severity::Warning);
    assert_eq!(diagnostic.subject, Subject::Input(Input::Install));
    assert_eq!(
        diagnostic.message(&Render::plain()),
        "the install does not say its build: no content-metadata.json"
    );
    assert_eq!(diagnostic.data()["why"], "metadata-missing");
}

#[test]
fn metadata_that_states_no_build_says_why() {
    let fixture = Fixture::standard();
    let why = |json: &str| {
        fixture.metadata(json);
        let install = fixture.open();
        assert_eq!(install.build(), None, "{json}");
        let log = raised(&install);
        let found = log.of::<VersionUnknown>().next().unwrap().why.clone();
        (found, log.iter().next().unwrap().message(&Render::plain()))
    };

    let (found, message) = why(r#"{ "version": "live" }"#);
    assert_eq!(
        found,
        NoBuild::VersionMalformed {
            version: Some("live".into())
        }
    );
    assert_eq!(
        message,
        "the install does not say its build: the version live is no build"
    );

    let (found, message) = why("{}");
    assert_eq!(found, NoBuild::VersionMalformed { version: None });
    assert_eq!(
        message,
        "the install does not say its build: content-metadata.json has no version"
    );

    let (found, message) = why("{ not json");
    assert!(
        matches!(found, NoBuild::MetadataUnreadable { .. }),
        "{found:?}"
    );
    assert!(
        message.starts_with(
            "the install does not say its build: content-metadata.json does not read: "
        ),
        "{message}"
    );
}

#[test]
fn a_wad_that_changed_since_the_install_was_opened_is_an_error() {
    let fixture = Fixture::standard();
    let install = fixture.open();
    let skin = path_hash(SKIN);

    fixture.wad("Champions/Lux.wad.client", &[(SHARED, b"glow of lux")]);
    let error = install.read(skin).unwrap_err();
    assert!(
        matches!(&error, GameError::Changed { path, .. } if *path == skin),
        "{error}"
    );
}

#[test]
fn a_wad_that_no_longer_mounts_is_an_error_at_the_read() {
    let fixture = Fixture::standard();
    let install = fixture.open();
    fixture.file("Champions/Lux.wad.client", b"not a WAD any more");
    let error = install.read(path_hash(SKIN)).unwrap_err();
    assert!(matches!(error, GameError::Mount { .. }), "{error}");
    // The next read mounts again.
    fixture.wad("Champions/Lux.wad.client", &[(SKIN, b"lux skin0")]);
    assert_eq!(
        install.read(path_hash(SKIN)).unwrap().unwrap(),
        b"lux skin0"
    );
}

#[test]
fn chunks_are_read_from_several_threads_at_once() {
    let fixture = Fixture::standard();
    let install = fixture.open();
    let chunks: [(&str, &[u8]); 3] = [
        (SKIN, b"lux skin0"),
        (SHARED, b"glow of ahri"),
        (SOUND, b"a bank"),
    ];
    std::thread::scope(|scope| {
        for _ in 0..8 {
            scope.spawn(|| {
                for _ in 0..50 {
                    for (path, bytes) in chunks {
                        assert_eq!(install.read(path_hash(path)).unwrap().unwrap(), bytes);
                    }
                }
            });
        }
    });
}

#[test]
fn the_fake_answers_as_an_install_does() {
    let fixture = Fixture::standard();
    let install = fixture.open();
    let mut fake = MemoryGame::new("Lux.wad.client", "16.19.8207193");
    fake.insert_in(
        SKIN,
        b"lux skin0".to_vec(),
        &["Lux.wad.client", "Audio.wad.client"],
    );
    fake.insert_in(
        SHARED,
        b"glow of ahri".to_vec(),
        &["Ahri.wad.client", "Lux.wad.client"],
    );
    fake.insert_in(SOUND, b"a bank".to_vec(), &["Audio.wad.client"]);

    let games: [&dyn Game; 2] = [&install, &fake];
    for path in [SKIN, SHARED, SOUND, "assets/none.tex"] {
        let hash = path_hash(path);
        let [real, fake] = games.map(|game| {
            (
                game.contains(hash),
                game.size(hash),
                game.read(hash).unwrap(),
                game.wads(hash),
            )
        });
        assert_eq!(real, fake, "{path}");
    }
    assert_eq!(install.build(), fake.build());
    assert_eq!(fake.describe(), "memory");

    // A chunk put in the fake with no WAD named is in its one champion WAD.
    fake.insert("data/characters/lux/lux.bin", b"root".to_vec());
    assert_eq!(
        names(&fake.wads(path_hash("data/characters/lux/lux.bin"))),
        ["Lux.wad.client"]
    );
}

#[test]
fn an_install_is_shared_between_threads() {
    fn shared<T: Send + Sync>() {}
    shared::<Install>();
    shared::<SizeIndex>();
}
