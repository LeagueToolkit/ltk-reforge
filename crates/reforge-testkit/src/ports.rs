//! One archive taken through every stage of a port: `load`, `fix`, `facts`, `decide`, `plan`,
//! the build of its package and the checks.

use reforge::diag::{Diag, Diagnostics, Render, Stage};
use reforge::env::Env;
use reforge::fixups::TargetTypes;
use reforge::model::entity::{ObjectId, UnitId};
use reforge::model::{ByteView, Dump, DumpCx, name_hash, path_hash};
use reforge::package::{
    AppliedBin, BuildError, CheckCx, CheckSet, Chunk, Layer, Outcome, Overlay, Package, Patch,
    PortBuilder,
};
use reforge::port::{Policy, Port, PortInputs, PortOptions, Targets};
use reforge::report::Report;

use crate::facts::FactRun;
use crate::mods::ModArchive;

/// A mod planned, built and checked as a port, with what the stages raised.
pub struct PortRun<'a> {
    env: &'a Env,
    /// The mod as the fixups left it, with what is true of it.
    pub facts: FactRun<'a>,
    /// Where each unit goes.
    pub targets: Targets,
    /// The plan.
    pub port: Port,
    /// The package, or why there is none.
    pub package: Result<Package, BuildError>,
    /// The package's patches applied to the game's bins.
    pub overlay: Overlay,
    /// Every diagnostic of `decide`, `plan`, `build` and `check`.
    pub log: Diagnostics,
}

impl<'a> PortRun<'a> {
    /// Ports an archive under the default policy and options, with the standard checks.
    ///
    /// # Panics
    ///
    /// When the archive does not load, or an input does not answer.
    pub fn of(env: &'a Env, archive: &ModArchive) -> Self {
        Self::under(env, archive, Policy::default(), PortOptions::default())
    }

    /// As [`PortRun::of`], under a policy and options of the caller's.
    ///
    /// # Panics
    ///
    /// As [`PortRun::of`].
    pub fn under(env: &'a Env, archive: &ModArchive, policy: Policy, options: PortOptions) -> Self {
        Self::from_facts(env, FactRun::of(env, archive), policy, options)
    }

    /// Ports a mod whose facts are gathered, with the names its run learnt.
    ///
    /// # Panics
    ///
    /// When an input does not answer.
    pub fn from_facts(
        env: &'a Env,
        mut facts: FactRun<'a>,
        policy: Policy,
        options: PortOptions,
    ) -> Self {
        let mut log = Diagnostics::new();
        let targets = policy.decide(&facts.facts);
        targets.report(&mut log.at(Stage::Decide));
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
            let port = Port::plan(&inputs, &mut log.at(Stage::Plan));
            port.unwrap_or_else(|error| panic!("the port is planned: {error}"))
        };
        let (paths, entries) = port.made();
        for path in paths {
            facts.fix.names.learn_path(path);
        }
        for entry in entries {
            facts.fix.names.learn_entry(entry);
        }
        let speller = facts.fix.speller();
        let builder = PortBuilder::new(&facts.fix.fixed, &port, env, &speller);
        let package = builder.build(&mut log.at(Stage::Build));
        let mut overlay = Overlay::default();
        if let Ok(package) = &package {
            overlay = Overlay::apply(package, env.game())
                .unwrap_or_else(|error| panic!("the game reads: {error}"));
            let types = TargetTypes::of(&facts.fix.fixed, env)
                .unwrap_or_else(|error| panic!("the game reads: {error}"));
            let cx = CheckCx::new(package, &types).of_port(env.game(), &overlay, &port);
            let ran = CheckSet::standard().run(&cx, &mut log.at(Stage::Check));
            ran.unwrap_or_else(|cancelled| panic!("a log with no cancel ended: {cancelled}"));
        }
        Self {
            env,
            facts,
            targets,
            port,
            package,
            overlay,
            log,
        }
    }

    /// The package.
    ///
    /// # Panics
    ///
    /// When the build gave none, with what the run raised.
    pub fn package(&self) -> &Package {
        match &self.package {
            Ok(package) => package,
            Err(error) => panic!("no package: {error}\n{:#?}", self.messages()),
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

    /// The object of the mod under an entry name, as the archive names it.
    ///
    /// # Panics
    ///
    /// When the mod has no such object.
    pub fn object(&self, entry: &str) -> ObjectId {
        let hash = name_hash(entry);
        let mut objects = self.facts.fix.fixed.objects.iter();
        let found = objects.find(|(_, object)| object.entry == hash);
        found
            .map(|(id, _)| id)
            .unwrap_or_else(|| panic!("no object {entry}\n{}", self.dump()))
    }

    /// The chunk at a path, in any layer and WAD.
    ///
    /// # Panics
    ///
    /// When the package holds none there.
    pub fn chunk(&self, path: &str) -> &Chunk {
        let chunk = self.package().by_hash(path_hash(path));
        chunk.unwrap_or_else(|| panic!("no chunk at {path}\n{}", self.dump()))
    }

    /// The bytes of the chunk at a path, in any layer and WAD.
    ///
    /// # Panics
    ///
    /// When the package holds none there.
    pub fn chunk_bytes(&self, path: &str) -> ByteView {
        self.package().bytes(self.chunk(path))
    }

    /// Whether the package holds a chunk at a path.
    pub fn ships(&self, path: &str) -> bool {
        self.package().by_hash(path_hash(path)).is_some()
    }

    /// The patch of a layer on a game bin.
    ///
    /// # Panics
    ///
    /// When the layer holds none on the bin.
    pub fn patch(&self, layer: &Layer, target: &str) -> &Patch {
        let mut patches = self.package().patches(layer).iter();
        let found = patches.find(|patch| patch.target.as_str() == target);
        found.unwrap_or_else(|| panic!("no patch on {target}\n{}", self.dump()))
    }

    /// The game bin at a path after the layer's patch of it.
    ///
    /// # Panics
    ///
    /// When the layer holds no patch on the bin, or the patch did not apply.
    pub fn applied(&self, layer: &Layer, target: &str) -> &AppliedBin {
        let mut patches = self.package().patches(layer).iter();
        let at = patches
            .position(|patch| patch.target.as_str() == target)
            .unwrap_or_else(|| panic!("no patch on {target}\n{}", self.dump()));
        match self.overlay.of(layer, at).map(|applied| &applied.outcome) {
            Some(Outcome::Applied(bin)) => bin,
            other => panic!("the patch on {target} did not apply: {other:?}"),
        }
    }

    /// The findings of one type, in the order raised.
    pub fn found<T: Diag>(&self) -> Vec<&T> {
        self.log.of::<T>().collect()
    }

    /// The one finding of a type.
    ///
    /// # Panics
    ///
    /// When the run raised none or several of the type.
    pub fn only<T: Diag>(&self) -> &T {
        let mut found = self.log.of::<T>();
        match (found.next(), found.next()) {
            (Some(finding), None) => finding,
            _ => panic!("not one {}:\n{:#?}", T::CODE, self.messages()),
        }
    }

    /// Every diagnostic of the port's stages as its code and its line for a person.
    pub fn messages(&self) -> Vec<String> {
        let speller = self.facts.fix.speller();
        let cx = Render {
            names: &speller,
            module: Some(&self.facts.fix.fixed),
        };
        let log = self.log.iter();
        log.map(|d| format!("{} {}", d.code, d.message(&cx)))
            .collect()
    }

    /// The codes of the errors the checks raised.
    pub fn check_errors(&self) -> Vec<String> {
        let errors = self.log.iter().filter(|d| {
            d.stage == Stage::Check && d.severity() >= reforge::diag::Severity::Warning
        });
        let speller = self.facts.fix.speller();
        let cx = Render {
            names: &speller,
            module: Some(&self.facts.fix.fixed),
        };
        errors
            .map(|d| format!("{} {}", d.code, d.message(&cx)))
            .collect()
    }

    /// The plan and the package as text.
    pub fn dump(&self) -> String {
        let speller = self.facts.fix.speller();
        let cx = DumpCx::new(&speller).of(&self.facts.fix.fixed);
        let mut out = self.port.dump(&cx);
        if let Ok(package) = &self.package {
            out.push_str(&package.dump(&cx));
        }
        out.push_str(&self.messages().join("\n"));
        out
    }

    /// The report of the run, as JSON.
    pub fn report(&self) -> String {
        let speller = self.facts.fix.speller();
        let m = &self.facts.fix.fixed;
        let cx = Render {
            names: &speller,
            module: Some(m),
        };
        let mut report = Report::builder()
            .env(self.env)
            .module(m)
            .facts(&self.facts.facts, self.env, &speller)
            .targets(&self.targets)
            .port(&self.port);
        if let Ok(package) = &self.package {
            report = report.package_applied(package, Some(&self.overlay));
        }
        report.diagnostics(&self.log, &cx).build().to_json()
    }
}
