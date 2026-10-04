//! One archive taken through `load`, `fix` and `facts`, for tests of the facts and of the
//! decision.

use reforge::diag::{Diag, Diagnostics, Stage};
use reforge::env::Env;
use reforge::facts::{FactInputs, Facts};
use reforge::fixups::FixupSet;
use reforge::game::Speller;
use reforge::load::{FileProvenance, LoadOptions, ReworkTable, against};
use reforge::model::entity::UnitId;
use reforge::model::{Dump, DumpCx};
use reforge::port::{Decision, Policy, Targets};

use ltk_meta::BinObject;
use reforge::history::History;

use crate::bins::{Skin, bin, blob, entry, graph};
use crate::fixes::FixRun;
use crate::game::MemoryGame;
use crate::mods::ModArchive;
use crate::skins::{WAD, skin_bin};

/// An environment of a game, its chunks by size and a history.
///
/// # Panics
///
/// When the environment does not build, which with a game given it does.
pub fn env(game: MemoryGame, history: impl History + 'static) -> Env {
    let sizes = game.sizes();
    let env = Env::builder()
        .game(game)
        .sizes(sizes)
        .history(Box::new(history));
    env.build().expect("a game was given")
}

/// The files of a model of the mod's own under `assets/mod/<prefix>/`: a mesh, a skeleton, a
/// texture and two clips. None of them reads as what its name says.
pub fn own_files(prefix: &str) -> Vec<(String, Vec<u8>)> {
    let names = [
        "mesh.skn",
        "skeleton.skl",
        "body.tex",
        "idle.anm",
        "dance.anm",
    ];
    let paths = names
        .iter()
        .map(|name| format!("assets/mod/{prefix}/{name}"));
    paths.map(|path| (path.clone(), blob(&path))).collect()
}

/// A skin entry on the model of [`own_files`], naming a graph and a resolver.
pub fn own_skin(prefix: &str, graph: &str, resolver: &str) -> Skin {
    Skin {
        mesh: format!("assets/mod/{prefix}/mesh.skn"),
        skeleton: format!("assets/mod/{prefix}/skeleton.skl"),
        texture: format!("assets/mod/{prefix}/body.tex"),
        graph: graph.into(),
        resolver: resolver.into(),
        name: "Test".into(),
        class: None,
        parent: None,
    }
}

/// A graph under an entry name that plays the two clips of [`own_files`].
pub fn own_graph(prefix: &str, entry_name: &str) -> BinObject {
    let clips = [
        format!("assets/mod/{prefix}/idle.anm"),
        format!("assets/mod/{prefix}/dance.anm"),
    ];
    graph(entry_name, &[clips[0].as_str(), clips[1].as_str()])
}

/// A skin bin of a character at a slot: its path, and a bin of an entry under `entry_name`
/// and the objects of `extra`.
pub fn mod_skin(
    character: &str,
    slot: u16,
    entry_name: &str,
    skin: &Skin,
    extra: Vec<BinObject>,
) -> (String, Vec<u8>) {
    let mut objects = vec![entry(entry_name, skin)];
    objects.extend(extra);
    (skin_bin(character, slot), bin(objects))
}

/// An archive of these files, in the WAD of the champion `test`.
pub fn archive(files: &[(String, Vec<u8>)]) -> ModArchive {
    let add = |archive: ModArchive, (path, bytes): &(String, Vec<u8>)| {
        archive.file(WAD, path, bytes.clone())
    };
    files.iter().fold(ModArchive::new(), add)
}

/// A mod loaded and fixed, with what is true of it.
pub struct FactRun<'a> {
    env: &'a Env,
    /// The mod as the fixups left it, with what they did.
    pub fix: FixRun<'a>,
    /// Whose bytes the mod's files are.
    pub files: FileProvenance,
    /// The facts.
    pub facts: Facts,
}

impl<'a> FactRun<'a> {
    /// Loads and fixes an archive with the standard fixups, and gathers its facts against the
    /// standard table of reworks.
    ///
    /// # Panics
    ///
    /// When the archive does not load, or an input does not answer.
    pub fn of(env: &'a Env, archive: &ModArchive) -> Self {
        Self::with_reworks(env, archive, &ReworkTable::standard())
    }

    /// As [`FactRun::of`], against a table of reworks of the caller's.
    ///
    /// # Panics
    ///
    /// As [`FactRun::of`].
    pub fn with_reworks(env: &'a Env, archive: &ModArchive, reworks: &ReworkTable) -> Self {
        let options = LoadOptions::default().reworks(reworks.clone());
        let fix = FixRun::under(
            env,
            &FixupSet::standard(),
            archive,
            (options, &|_| reforge::fixups::ObjectSet::every()),
        );
        let speller = Speller::new(&fix.names, env.names());
        let files = FileProvenance::find(&fix.fixed, &against(env, &speller));
        let files = files.unwrap_or_else(|error| panic!("the files' provenance is found: {error}"));
        let inputs = FactInputs {
            env,
            dating: &fix.dating,
            files: &files,
            names: &speller,
            reworks,
        };
        let facts = Facts::gather(&fix.fixed, &inputs);
        let facts = facts.unwrap_or_else(|error| panic!("the facts are gathered: {error}"));
        Self {
            env,
            fix,
            files,
            facts,
        }
    }

    /// The unit of a character at a slot.
    ///
    /// # Panics
    ///
    /// When the mod has no such unit.
    pub fn unit(&self, character: &str, slot: u16) -> UnitId {
        let m = &self.fix.fixed;
        let found = m.units.iter().find(|(_, unit)| {
            m.characters[unit.character].name == character && unit.source_slot.0 == slot
        });
        found
            .map(|(id, _)| id)
            .unwrap_or_else(|| panic!("no unit {character} skin{slot}\n{}", self.dump()))
    }

    /// The decision of a unit under a policy's targets.
    ///
    /// # Panics
    ///
    /// When the unit has none.
    pub fn decision<'t>(&self, targets: &'t Targets, character: &str, slot: u16) -> &'t Decision {
        let unit = self.unit(character, slot);
        let decision = targets.decisions.get(&unit);
        decision
            .unwrap_or_else(|| panic!("no decision for {character} skin{slot}\n{}", self.dump()))
    }

    /// The decision of a unit under a policy.
    ///
    /// # Panics
    ///
    /// When the unit has none.
    pub fn decide(&self, policy: Policy, character: &str, slot: u16) -> Decision {
        self.decision(&policy.decide(&self.facts), character, slot)
            .clone()
    }

    /// What the facts raise, in a log of their own.
    pub fn raised(&self) -> Diagnostics {
        let mut log = Diagnostics::new();
        self.facts.report(&mut log.at(Stage::Facts));
        log
    }

    /// What a policy's decisions raise, in a log of their own.
    pub fn decided(&self, policy: Policy) -> Diagnostics {
        let mut log = Diagnostics::new();
        policy
            .decide(&self.facts)
            .report(&mut log.at(Stage::Decide));
        log
    }

    /// The findings of one type among what the facts raise.
    pub fn found<T: Diag + Clone>(&self) -> Vec<T> {
        self.raised().of::<T>().cloned().collect()
    }

    /// The facts as text.
    pub fn dump(&self) -> String {
        let speller = Speller::new(&self.fix.names, self.env.names());
        self.facts.dump(&DumpCx::new(&speller).of(&self.fix.fixed))
    }
}
