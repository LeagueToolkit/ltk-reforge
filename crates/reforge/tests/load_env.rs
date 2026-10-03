//! The environment a run is judged against, and the view of the game that keeps what it was
//! asked. The game is `reforge_testkit::skins`.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use reforge::env::{Env, EnvError};
use reforge::game::{Build, Game, GameError, GameView};
use reforge::load::{LoadError, LoadOptions};
use reforge::model::{ContentId, Slot, WadHash, WadName, path_hash};
use reforge_testkit::bins::{bin, blob, entry, graph, linked_bin};
use reforge_testkit::skins::{self, BASE_CLIPS, BASE_MESH, WAD, base_skin, skin_bin};
use reforge_testkit::{LoadRun, MemoryGame, MemoryHistory, ModArchive};

/// A game that counts the chunks read from it, and fails every read while it is broken.
struct Counting {
    game: MemoryGame,
    reads: Arc<AtomicUsize>,
    broken: Arc<AtomicBool>,
}

impl Game for Counting {
    fn build(&self) -> Option<&Build> {
        self.game.build()
    }

    fn describe(&self) -> String {
        self.game.describe()
    }

    fn size(&self, path: WadHash) -> Option<u64> {
        self.game.size(path)
    }

    fn read(&self, path: WadHash) -> Result<Option<Vec<u8>>, GameError> {
        self.reads.fetch_add(1, Ordering::Relaxed);
        if self.broken.load(Ordering::Relaxed) {
            return Err(GameError::Open(std::io::Error::other("the WAD is gone")));
        }
        self.game.read(path)
    }

    fn wads(&self, path: WadHash) -> Vec<WadName> {
        self.game.wads(path)
    }
}

/// A view of a game, the count of its reads, and the switch that breaks them.
fn counting(game: MemoryGame) -> (GameView, Arc<AtomicUsize>, Arc<AtomicBool>) {
    let reads = Arc::new(AtomicUsize::new(0));
    let broken = Arc::new(AtomicBool::new(false));
    let game = Counting {
        game,
        reads: Arc::clone(&reads),
        broken: Arc::clone(&broken),
    };
    (GameView::new(Box::new(game)), reads, broken)
}

#[test]
fn an_environment_needs_a_game_and_nothing_else() {
    assert_eq!(Env::builder().build().err(), Some(EnvError::NoGame));
    let env = Env::builder()
        .game(skins::game())
        .build()
        .expect("an environment");
    assert!(!env.has_history());
    assert!(env.history().builds().is_empty());
    assert!(env.schema().is_none());
    assert_eq!(env.names().path(path_hash(BASE_MESH)), None);
    assert_eq!(env.game().build().map(Build::as_str), Some("16.19.1"));
    let wad = WadName::parse(skins::WAD).expect("a WAD name");
    assert!(
        env.sizes()
            .same_size(&wad, blob(BASE_MESH).len() as u64)
            .is_empty()
    );
}

#[test]
fn an_environment_holds_what_it_was_given() {
    let game = skins::game();
    let sizes = game.sizes();
    let history = MemoryHistory::new(&["16.18.1", "16.19.1"]);
    let env = Env::builder()
        .game(game)
        .sizes(sizes)
        .history(Box::new(history));
    let env = env.build().expect("an environment");
    assert!(env.has_history());
    assert_eq!(env.history().builds().len(), 2);
    let wad = WadName::parse(skins::WAD).expect("a WAD name");
    let same = env.sizes().same_size(&wad, blob(BASE_MESH).len() as u64);
    assert!(same.contains(&path_hash(BASE_MESH)));
}

#[test]
fn the_view_answers_as_its_game_does() {
    let (view, ..) = counting(skins::game());
    let mesh = path_hash(BASE_MESH);
    assert_eq!(view.size(mesh), Some(blob(BASE_MESH).len() as u64));
    assert!(view.contains(mesh));
    assert!(!view.contains(path_hash("assets/nowhere.tex")));
    assert_eq!(view.read(mesh).expect("it reads"), Some(blob(BASE_MESH)));
    assert_eq!(
        view.wad_of(mesh).map(|w| w.to_string()).as_deref(),
        Some(skins::WAD)
    );
    assert_eq!(view.wad_of(path_hash("assets/nowhere.tex")), None);
    assert_eq!(view.describe(), "memory");
}

#[test]
fn a_content_id_is_read_once() {
    let (view, reads, _) = counting(skins::game());
    let mesh = path_hash(BASE_MESH);
    let size = blob(BASE_MESH).len() as u64;
    let content = ContentId::of(&blob(BASE_MESH));
    assert_eq!(view.content(mesh).expect("it reads"), Some(content));
    assert_eq!(view.content(mesh).expect("it is kept"), Some(content));
    assert!(view.holds(mesh, size, content).expect("it is kept"));
    assert_eq!(reads.load(Ordering::Relaxed), 1);

    // Another size is told without a read, and an absent path is asked once.
    assert!(!view.holds(mesh, 1, content).expect("nothing is read"));
    let nowhere = path_hash("assets/nowhere.tex");
    assert_eq!(view.content(nowhere).expect("the game answers"), None);
    assert_eq!(view.content(nowhere).expect("it is kept"), None);
    assert_eq!(reads.load(Ordering::Relaxed), 2);
}

#[test]
fn a_bin_is_parsed_once_with_the_files_it_names() {
    let (view, reads, _) = counting(skins::game());
    let path = path_hash(&skin_bin("test", 0));
    let first = view
        .bin(path)
        .expect("it reads")
        .expect("the base skin bin");
    let second = view
        .bin(path)
        .expect("it is kept")
        .expect("the base skin bin");
    assert!(Arc::ptr_eq(&first, &second));
    assert_eq!(reads.load(Ordering::Relaxed), 1);
    assert_eq!(first.parsed.objects.len(), 4);
    assert!(first.files.contains(&path_hash(BASE_MESH)));
    assert!(first.files.contains(&path_hash(BASE_CLIPS[0])));
    assert_eq!(
        view.content(path).expect("it reads"),
        Some(ContentId::of(&first.bytes))
    );

    // A file that is no bin, and a path the game lacks.
    assert!(view.bin(path_hash(BASE_MESH)).expect("it reads").is_none());
    assert!(
        view.bin(path_hash("data/nowhere.bin"))
            .expect("the game answers")
            .is_none()
    );
}

#[test]
fn a_skin_is_its_skin_bin_and_the_bins_it_links_in_the_characters_folder() {
    const ANIMATIONS: &str = "data/characters/test/animations/skin0.bin";
    const SHARED: &str = "data/shared/common.bin";
    let mut game = skins::game();
    let links = [
        "DATA/Characters/Test/Animations/Skin0.bin",
        "DATA/Characters/Test/Test.bin",
        "DATA/Shared/Common.bin",
        "DATA/Characters/Test/Gone.bin",
    ];
    let objects = vec![entry("Characters/Test/Skins/Skin0", &base_skin())];
    game.insert(&skin_bin("test", 0), linked_bin(objects, &links));
    game.insert(
        ANIMATIONS,
        bin(vec![graph("Characters/Test/Animations/Skin0", BASE_CLIPS)]),
    );
    game.insert("data/characters/test/test.bin", bin(vec![]));
    game.insert(SHARED, bin(vec![]));
    let (view, reads, _) = counting(game);
    let skin = |character: &str, slot: u16| view.skin(character, Slot(slot)).expect("it reads");

    let base = skin("Test", 0).expect("the base skin");
    let paths: Vec<&str> = base.bins.iter().map(|(path, _)| path.as_str()).collect();
    // Not the root bin, not a bin of another folder, not a link the game does not hold.
    assert_eq!(paths, [skin_bin("test", 0).as_str(), ANIMATIONS]);
    assert!(!base.chroma);
    assert!(base.bins[1].1.files.contains(&path_hash(BASE_CLIPS[1])));
    let before = reads.load(Ordering::Relaxed);
    assert!(Arc::ptr_eq(&base, &skin("test", 0).expect("kept")));
    assert_eq!(reads.load(Ordering::Relaxed), before);

    assert!(skin("test", 9).expect("skin 9").chroma);
    assert!(!skin("test", 3).expect("skin 3").chroma);
    assert!(skin("test", 1).is_none());
    assert!(skin("nobody", 0).is_none());
}

#[test]
fn a_chunk_that_does_not_read_fails_every_question_and_is_asked_again() {
    let (view, reads, broken) = counting(skins::game());
    let mesh = path_hash(BASE_MESH);
    let size = blob(BASE_MESH).len() as u64;
    let content = ContentId::of(&blob(BASE_MESH));
    broken.store(true, Ordering::Relaxed);
    assert!(matches!(view.content(mesh), Err(GameError::Open(_))));
    assert!(matches!(
        view.holds(mesh, size, content),
        Err(GameError::Open(_))
    ));
    assert!(view.bin(path_hash(&skin_bin("test", 0))).is_err());
    assert!(view.skin("test", Slot(0)).is_err());
    // What the game's table of contents answers needs no read.
    assert!(!view.holds(mesh, size + 1, content).expect("another size"));
    assert_eq!(reads.load(Ordering::Relaxed), 4);

    // The failure is not kept as an answer.
    broken.store(false, Ordering::Relaxed);
    assert_eq!(view.content(mesh).expect("it reads"), Some(content));
    assert!(view.skin("test", Slot(0)).expect("it reads").is_some());
}

/// A game that reads nothing: every chunk it lists fails.
struct Unreadable(MemoryGame);

impl Game for Unreadable {
    fn build(&self) -> Option<&Build> {
        self.0.build()
    }

    fn describe(&self) -> String {
        self.0.describe()
    }

    fn size(&self, path: WadHash) -> Option<u64> {
        self.0.size(path)
    }

    fn read(&self, _: WadHash) -> Result<Option<Vec<u8>>, GameError> {
        Err(GameError::Open(std::io::Error::other("the WAD is gone")))
    }

    fn wads(&self, path: WadHash) -> Vec<WadName> {
        self.0.wads(path)
    }
}

#[test]
fn a_game_that_does_not_read_ends_the_load_with_a_fault() {
    let env = Env::builder()
        .game(Unreadable(skins::game()))
        .build()
        .expect("an environment");
    // A binless mod: its file is compared with the game's bytes at its path.
    let archive = ModArchive::new().file(
        WAD,
        BASE_MESH,
        b"the mod's own mesh, of the same size".to_vec(),
    );
    let run = LoadRun::of(&env, LoadOptions::new(), &archive);
    assert!(
        run.log.is_empty(),
        "a fault is no diagnostic: {:?}",
        run.codes()
    );
    // The mesh is of another size than the game's, so no read tells them apart: the skin bins
    // of the game are what does not read.
    assert!(matches!(
        run.result,
        Err(LoadError::Game(GameError::Open(_)))
    ));

    // A sub-character's skin bin of the game's size is compared by its bytes.
    let pet = skins::pet_bin();
    let mut other = pet.clone();
    let last = other.len() - 1;
    other[last] ^= 1;
    let archive = ModArchive::new()
        .file(WAD, &skin_bin("test", 0), bin(vec![]))
        .file(WAD, &skin_bin("testpet", 0), other);
    let run = LoadRun::of(&env, LoadOptions::new(), &archive);
    assert!(matches!(run.result, Err(LoadError::Game(_))));
}

#[test]
fn the_view_is_shared_between_threads() {
    fn shared<T: Send + Sync>(_: &T) {}
    let (view, reads, _) = counting(skins::game());
    shared(&view);
    let path = path_hash(&skin_bin("test", 3));
    std::thread::scope(|scope| {
        for _ in 0..4 {
            scope.spawn(|| assert!(view.bin(path).expect("it reads").is_some()));
        }
    });
    // Two threads may read one bin at once. Afterwards it is kept.
    let after = reads.load(Ordering::Relaxed);
    assert!((1..=4).contains(&after));
    assert!(view.bin(path).expect("it is kept").is_some());
    assert_eq!(reads.load(Ordering::Relaxed), after);
}
