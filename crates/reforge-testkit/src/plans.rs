//! One archive taken through `load`, `fix`, `facts`, `decide` and `plan`, for tests of the
//! plan's steps: no package is built. And the mods the tests of the decision and of the plan
//! share: the ones whose units leave the port.

use reforge::diag::{Diag, Diagnostics, Render, Stage};
use reforge::env::Env;
use reforge::load::ReworkTable;
use reforge::model::entity::UnitId;
use reforge::model::{Dump, DumpCx};
use reforge::port::{Left, PlanError, Policy, Port, PortInputs, PortOptions, Targets};

use crate::bins::{Skin, blob};
use crate::facts::{FactRun, mod_skin, own_files, own_graph, own_skin};
use crate::history::MemoryHistory;
use crate::mods::ModArchive;
use crate::{rigs, skins};

const SKIN0: &str = "Characters/Test/Skins/Skin0";
const SKIN3: &str = "Characters/Test/Skins/Skin3";
const PET_SKIN0: &str = "Characters/TestPet/Skins/Skin0";
const GRAPH0: &str = "Characters/Test/Animations/Skin0";
const GRAPH3: &str = "Characters/Test/Animations/Skin3";

/// The champion pasted from skin 3 in its skin0 bin, beside its sub-character on the
/// sub-character's base model with a texture of the mod's own.
pub fn champion_and_pet() -> Vec<(String, Vec<u8>)> {
    let texture = "assets/mod/pet/body.tex";
    let pet = Skin {
        texture: texture.into(),
        ..skins::pet_skin()
    };
    vec![
        mod_skin("test", 0, SKIN0, &skins::skin3(), vec![]),
        (texture.into(), blob(texture)),
        mod_skin("testpet", 0, PET_SKIN0, &pet, vec![]),
    ]
}

/// A paste of skin 3 in the skin0 bin, and the mod's own rig written for slot 3: two units
/// that both go to skin 3.
pub fn paste_and_own_skin3() -> Vec<(String, Vec<u8>)> {
    let mut files = own_files("a");
    files.push(mod_skin("test", 0, SKIN0, &skins::skin3(), vec![]));
    let own = own_skin("a", GRAPH3, "Characters/Test/Skins/Skin3/Resources");
    let graph = vec![own_graph("a", GRAPH3)];
    files.push(mod_skin("test", 3, SKIN3, &own, graph));
    files
}

/// The champion on the mod's own rig at skin0, and its sub-character on a mesh of the mod's
/// that binds a joint its own skeleton lacks: the sub-character's model fits no slot.
pub fn champion_and_unbound_pet() -> Vec<(String, Vec<u8>)> {
    let resolver = "Characters/Test/Skins/Skin0/Resources";
    let champion = own_skin("a", GRAPH0, resolver);
    let (mesh, skeleton) = ("assets/mod/pet/pet.skn", "assets/mod/pet/pet.skl");
    let pet = Skin {
        mesh: mesh.into(),
        skeleton: skeleton.into(),
        ..skins::pet_skin()
    };
    let mut files = own_files("a");
    files.push(mod_skin(
        "test",
        0,
        SKIN0,
        &champion,
        vec![own_graph("a", GRAPH0)],
    ));
    // Every vertex binds the joint at index 5, and the skeleton has two.
    files.push((mesh.into(), rigs::mesh(&[("body", &[[0.0, 0.0, 0.0]])], 5)));
    files.push((skeleton.into(), rigs::skeleton(&["Root", "Spine"])));
    files.push(mod_skin("testpet", 0, PET_SKIN0, &pet, vec![]));
    files
}

/// A texture of the champion's base model that the game dropped at its rework.
pub const OLD_TEX: &str = "assets/characters/test/skins/base/test_old.tex";
/// A texture of the champion's base model that the game ships since its rework.
pub const NEW_TEX: &str = "assets/characters/test/skins/base/test_new.tex";

/// A history of two builds in which `test` is reworked at the second: the path of
/// [`OLD_TEX`] is dropped at it, and the path of [`NEW_TEX`] ships from it.
pub fn reworked() -> (MemoryHistory, ReworkTable) {
    let history = MemoryHistory::new(&["16.18.1", "16.19.1"]);
    let history = history.with_path(OLD_TEX, 0..=0).with_path(NEW_TEX, 1..=1);
    (history, ReworkTable::new([("test", "16.19")]))
}

/// The champion on the game's base model with [`OLD_TEX`] as its texture, shipped, beside its
/// sub-character: a mod made for the model before the rework of [`reworked`].
pub fn old_model_and_pet() -> Vec<(String, Vec<u8>)> {
    let skin = Skin {
        texture: OLD_TEX.into(),
        ..skins::base_skin()
    };
    let mut files = champion_and_pet();
    files[0] = mod_skin("test", 0, SKIN0, &skin, vec![]);
    files.push((OLD_TEX.into(), blob(OLD_TEX)));
    files
}

/// A mod with its targets and the plan of its port, with what `plan` raised.
pub struct PlanRun<'a> {
    /// The mod as the fixups left it, with what is true of it.
    pub facts: FactRun<'a>,
    /// Where each unit goes.
    pub targets: Targets,
    /// The plan, or the input that did not answer.
    pub port: Result<Port, PlanError>,
    /// Every diagnostic of `plan`.
    pub log: Diagnostics,
}

impl<'a> PlanRun<'a> {
    /// Plans the port of an archive under the default policy and options.
    ///
    /// # Panics
    ///
    /// When the archive does not load, or an input does not answer before `plan`.
    pub fn of(env: &'a Env, archive: &ModArchive) -> Self {
        let facts = FactRun::of(env, archive);
        let targets = Policy::default().decide(&facts.facts);
        Self::decided(env, facts, targets, PortOptions::default())
    }

    /// Plans the port of a mod whose facts are gathered, to targets of the caller's. `env` is
    /// what `plan` is judged against, which a test of a fault makes another than the facts'.
    pub fn decided(env: &Env, facts: FactRun<'a>, targets: Targets, options: PortOptions) -> Self {
        let mut log = Diagnostics::new();
        let port = {
            let speller = facts.fix.speller();
            let inputs = PortInputs {
                fixed: &facts.fix.fixed,
                facts: &facts.facts,
                targets: &targets,
                env,
                files: &facts.files,
                names: &speller,
                options: &options,
            };
            Port::plan(&inputs, &mut log.at(Stage::Plan))
        };
        Self {
            facts,
            targets,
            port,
            log,
        }
    }

    /// The plan.
    ///
    /// # Panics
    ///
    /// When `plan` gave none.
    pub fn port(&self) -> &Port {
        match &self.port {
            Ok(port) => port,
            Err(error) => panic!("no plan: {error}"),
        }
    }

    /// The unit of a character at a slot.
    ///
    /// # Panics
    ///
    /// When the mod has no such unit.
    pub fn unit(&self, character: &str, slot: u16) -> UnitId {
        self.facts.unit(character, slot)
    }

    /// Why the unit of a character at a slot leaves the port. None for a unit that ships.
    ///
    /// # Panics
    ///
    /// When the mod has no such unit, or `plan` gave no plan.
    pub fn left(&self, character: &str, slot: u16) -> Option<Left> {
        let left = self.port().shipping.left.get(&self.unit(character, slot));
        left.map(|left| left.why)
    }

    /// The findings of one type, in the order raised.
    pub fn found<T: Diag>(&self) -> Vec<&T> {
        self.log.of::<T>().collect()
    }

    /// The one finding of a type.
    ///
    /// # Panics
    ///
    /// When `plan` raised none or several of the type.
    pub fn only<T: Diag>(&self) -> &T {
        let mut found = self.log.of::<T>();
        match (found.next(), found.next()) {
            (Some(finding), None) => finding,
            _ => panic!("not one {}:\n{}", T::CODE, self.dump()),
        }
    }

    /// The plan as text, then every diagnostic of `plan` as its code and its line for a
    /// person.
    pub fn dump(&self) -> String {
        let speller = self.facts.fix.speller();
        let module = &self.facts.fix.fixed;
        let mut out = match &self.port {
            Ok(port) => port.dump(&DumpCx::new(&speller).of(module)),
            Err(error) => format!("no plan: {error}\n"),
        };
        let cx = Render {
            names: &speller,
            module: Some(module),
        };
        for raised in self.log.iter() {
            out.push_str(&format!("{} {}\n", raised.code, raised.message(&cx)));
        }
        out
    }
}
