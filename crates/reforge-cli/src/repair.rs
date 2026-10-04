//! `reforge repair`: the mod as it is, at its own slots and paths, with its bins fixed for
//! the target build (#ADR-0011). The stages in order: read, load, fix, build in place, check,
//! write. The report holds what each found.

use std::collections::BTreeSet;
use std::io::{Cursor, Write as _};
use std::process::ExitCode;

use anyhow::{Context, Result, bail};
use camino::Utf8PathBuf;
use clap::Args;
use fs_err as fs;
use reforge::archive::{Archive, Caps};
use reforge::diag::{Action, DiagnosticPolicy, Diagnostics, Filter, Render, Stage};
use reforge::env::Env;
use reforge::fixups::{Changes, FixInputs, FixedMod, FixupSet, ObjectSet, TargetTypes};
use reforge::game::{NameBook, Speller};
use reforge::load::{Dating, LoadOptions, Loader};
use reforge::model::{Dump, DumpCx, Mod};
use reforge::package::{
    BuildError, CheckCx, CheckSelection, CheckSet, InPlaceBuilder, Modpkg, Package, PackageWriter,
    ProjectDir, permit,
};
use reforge::report::{AgainRow, Report, ReportBuilder};
use zip::CompressionMethod;
use zip::write::SimpleFileOptions;

use crate::fixups::Choice;
use crate::run::{self, HALTED, Inputs, SchemaInput};

/// The exit code of a repair whose write the caller's policy refused.
const BLOCKED: u8 = 3;

#[derive(Args)]
pub struct Repair {
    /// The mod archive: a `.fantome`, or a zip of the same layout.
    archive: Utf8PathBuf,
    #[command(flatten)]
    inputs: Inputs,
    /// The directory the repaired mod is written in: a mod project, `<out>/<project>`.
    #[arg(long, required_unless_present = "no_write")]
    out: Option<Utf8PathBuf>,
    /// Also pack the project, as `<out>/<project>_<version>.modpkg`.
    #[arg(long)]
    modpkg: bool,
    /// Do everything but the write.
    #[arg(long)]
    no_write: bool,
    /// Write the report, as JSON, to this file.
    #[arg(long)]
    report: Option<Utf8PathBuf>,
    /// Take the package through the run a second time, in memory, and report what the second
    /// repair changed: nothing, for a repair that is whole.
    #[arg(long)]
    again: bool,
    /// Print the package as built.
    #[arg(long)]
    dump: bool,
    /// Carry stale copies as the mod's own files.
    #[arg(long)]
    keep_stale: bool,
    /// The meta schema database the fixups and the checks read the target build's types from
    /// (default: the one LTK Manager keeps on this machine).
    #[arg(long)]
    schema: Option<Utf8PathBuf>,
    #[command(flatten)]
    choice: Choice,
    /// Turn a check off by its id: `types`, `round-trip`. Repeatable.
    #[arg(long = "no-check")]
    no_check: Vec<String>,
    /// Refuse the write while the log holds a diagnostic this filter selects: codes or areas
    /// and `key=value` terms, separated by commas (`check,severity=error`). Repeatable.
    #[arg(long)]
    block: Vec<String>,
}

/// A mod loaded, dated and fixed.
struct Fixed {
    module: FixedMod,
    changes: Changes,
    dating: Dating,
}

/// The mod as far as the run took it.
enum Reached {
    /// The archive did not read.
    Nothing,
    /// `load` ended at a fatal diagnostic: the mod as far as it went.
    Halted(Box<Mod>),
    /// The mod as the fixups left it.
    Fixed(Box<Fixed>),
}

impl Reached {
    fn module(&self) -> Option<&Mod> {
        match self {
            Reached::Nothing => None,
            Reached::Halted(module) => Some(module),
            Reached::Fixed(fixed) => Some(&fixed.module),
        }
    }
}

/// What the stages of a repair are run with.
struct Stages<'a> {
    env: &'a Env,
    set: &'a FixupSet,
    keep_stale: bool,
}

impl Stages<'_> {
    /// The options `load` runs under, and the package's textures are judged under.
    fn options(&self) -> LoadOptions {
        LoadOptions::new()
            .keep_stale(self.keep_stale)
            .file_fixups(self.set.file_fixups())
    }

    /// Loads an archive, with the WADs an earlier run of the mod found to restore a model.
    /// The run's names are the ones `load` learnt.
    fn load(
        &self,
        archive: Archive,
        restored: &BTreeSet<String>,
        book: &mut NameBook,
        log: &mut Diagnostics,
    ) -> Result<run::Loaded> {
        let mut loader = Loader::new(self.env).options(self.options());
        let mut sink = log.at(Stage::Load);
        let loaded = run::load(&mut loader, archive, restored, &mut sink)?;
        *book = loader.into_names();
        Ok(loaded)
    }

    /// Applies the fixups to a loaded mod, judged at a dating.
    fn fix(
        &self,
        module: Mod,
        dating: Dating,
        book: &mut NameBook,
        log: &mut Diagnostics,
    ) -> Result<Reached> {
        let mut inputs = FixInputs {
            env: self.env,
            dating: &dating,
            names: book,
        };
        let mut sink = log.at(Stage::Fix);
        let (module, changes) = self
            .set
            .apply(module, &ObjectSet::every(), &mut inputs, &mut sink)
            .context("an input of the run did not answer while the mod was fixed")?;
        Ok(Reached::Fixed(Box::new(Fixed {
            module,
            changes,
            dating,
        })))
    }

    /// Loads an archive, dates the mod and applies the fixups.
    fn repair(
        &self,
        archive: Archive,
        book: &mut NameBook,
        log: &mut Diagnostics,
    ) -> Result<Reached> {
        let loaded = self.load(archive, &BTreeSet::new(), book, log)?;
        if loaded.halted {
            return Ok(Reached::Halted(Box::new(loaded.module)));
        }
        let dating = Dating::find(&loaded.module, self.env.history()).context("dating the mod")?;
        self.fix(loaded.module, dating, book, log)
    }

    /// The repair's package of a fixed mod. None where the run did not reach one, or it would
    /// hold nothing.
    fn build(
        &self,
        reached: &Reached,
        book: &NameBook,
        log: &mut Diagnostics,
    ) -> Result<Option<Package>> {
        let Reached::Fixed(fixed) = reached else {
            return Ok(None);
        };
        let options = self.options();
        let builder = InPlaceBuilder::new(&fixed.module, &fixed.changes)
            .names(book, self.env.names())
            .judged(self.env, &options);
        match builder.build(&mut log.at(Stage::Build)) {
            Ok(package) => Ok(Some(package)),
            Err(BuildError::Halted(_)) => Ok(None),
            Err(fault) => Err(fault).context("building the package"),
        }
    }

    /// Takes a package through `read`, `load`, `fix` and the build again, with a log of its
    /// own, and says what the second run changed.
    ///
    /// The second run is given what the first established of the mod and its package does
    /// not show again: the dating, and the WADs that restore a model from before its rework.
    /// An object a fixup brought to the target build is the game's own object of that build,
    /// so a repaired mod dates later by itself. A restoration is known by its files at paths
    /// the rework dropped, which a package leaves out where nothing names them (#ADR-0039).
    fn again(&self, first: &Package, fixed: &Fixed) -> Result<AgainRow> {
        let bytes = archive_of(first).context("the package as an archive in memory")?;
        let mut log = Diagnostics::new();
        let mut book = NameBook::new();
        let caps = Caps::default();
        let read = Archive::read(&bytes, "repaired.fantome", &caps, &mut log.at(Stage::Read));
        let Ok(archive) = read else {
            return Ok(AgainRow::of(first, None, None));
        };
        let restored = fixed.module.restorations.iter();
        let restored: BTreeSet<String> = restored
            .map(|&wad| fixed.module.wads[wad].name.lower().to_string())
            .collect();
        let loaded = self.load(archive, &restored, &mut book, &mut log)?;
        if loaded.halted {
            return Ok(AgainRow::of(first, None, None));
        }
        let own = Dating::find(&loaded.module, self.env.history()).context("dating the mod")?;
        let reached = self.fix(loaded.module, fixed.dating.clone(), &mut book, &mut log)?;
        let second = self.build(&reached, &book, &mut log)?;
        let changes = match &reached {
            Reached::Fixed(fixed) => Some(&fixed.changes),
            _ => None,
        };
        Ok(AgainRow::of(first, second.as_ref(), changes).redated(&own))
    }
}

/// A package's chunks as the bytes of a mod archive, each stored at `WAD/<wad>/<path>`. In
/// memory: nothing of the mod is written for the second run (#ADR-0024).
fn archive_of(package: &Package) -> Result<Vec<u8>> {
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (_, key, chunk) in package.chunks() {
        let options = SimpleFileOptions::default()
            .compression_method(CompressionMethod::Stored)
            .large_file(chunk.bytes.len() >= u32::MAX as usize);
        zip.start_file(format!("WAD/{}/{}", key.wad, key.path), options)?;
        zip.write_all(&chunk.bytes)?;
    }
    Ok(zip.finish()?.into_inner())
}

impl Repair {
    pub fn run(&self) -> Result<ExitCode> {
        let mut log = Diagnostics::new();
        let set = FixupSet::from(&self.choice.selection()?);
        let checks = CheckSet::from(&self.checks()?);
        let policy = self.policy()?;
        let schema = SchemaInput::Read(self.schema.as_deref());
        let env = self.inputs.env(schema, &mut log.at(Stage::Read))?;

        let stages = Stages {
            env: &env,
            set: &set,
            keep_stale: self.keep_stale,
        };
        let mut book = NameBook::new();
        let caps = Caps::default();
        let reached = match Archive::read_file(&self.archive, &caps, &mut log.at(Stage::Read)) {
            Ok(archive) => stages.repair(archive, &mut book, &mut log)?,
            Err(_) => Reached::Nothing,
        };
        let package = stages.build(&reached, &book, &mut log)?;
        if let (Reached::Fixed(fixed), Some(package)) = (&reached, &package) {
            let types = TargetTypes::of(&fixed.module, &env)
                .context("the game did not read while the package was checked")?;
            let cx = CheckCx {
                package,
                types: &types,
            };
            checks.run(&cx, &mut log.at(Stage::Check));
        }

        let speller = Speller::new(&book, env.names());
        let cx = Render {
            names: &speller,
            module: reached.module(),
        };
        let mut report = Self::report(&env, &reached, package.as_ref(), &speller);
        let again = match (&reached, &package, self.again) {
            (Reached::Fixed(fixed), Some(package), true) => Some(stages.again(package, fixed)?),
            _ => None,
        };
        if let Some(again) = &again {
            report = report.again(again.clone());
        }
        let mut refused = false;
        if let (Some(package), Some(out)) = (&package, self.out.as_ref().filter(|_| !self.no_write))
        {
            match permit(&policy, &mut log) {
                Ok(()) => {
                    // The report beside the package is the run as it stands before the write.
                    let text = report.clone().diagnostics(&log, &cx).build().to_json() + "\n";
                    self.write(package, out, &text, &mut log)?;
                }
                Err(_) => refused = true,
            }
        }

        run::print(&log, &cx, &policy);
        if let Some(again) = &again {
            outln!("{}", Self::again_line(again));
        }
        if let (true, Some(package)) = (self.dump, &package) {
            out!("{}", package.dump(&DumpCx::new(&speller)));
        }
        if let Some(path) = &self.report {
            let report = report.diagnostics(&log, &cx).build();
            fs::write(path, report.to_json() + "\n")?;
        }
        Ok(match (log.halted(), refused) {
            (Some(_), _) => ExitCode::from(HALTED),
            (None, true) => ExitCode::from(BLOCKED),
            (None, false) => ExitCode::SUCCESS,
        })
    }

    /// The checks the command line leaves on. An id that names no check is refused.
    fn checks(&self) -> Result<CheckSelection> {
        let mut selection = CheckSelection::standard();
        for id in &self.no_check {
            selection = selection.without(id.as_str());
        }
        if let Some(unknown) = selection.unknown().first() {
            let known: Vec<String> = CheckSet::catalog()
                .iter()
                .map(|info| info.id.to_string())
                .collect();
            bail!("{unknown} is no check: {}", known.join(", "));
        }
        Ok(selection)
    }

    /// The policy the command line gives: nothing blocks unless a filter says so.
    fn policy(&self) -> Result<DiagnosticPolicy> {
        let mut policy = DiagnosticPolicy::default();
        for text in &self.block {
            let filter = Filter::parse(text).with_context(|| format!("the filter {text:?}"))?;
            policy = policy.with(filter, Action::Block);
        }
        Ok(policy)
    }

    /// What the second repair came to, on one line.
    fn again_line(again: &AgainRow) -> String {
        if again.same() {
            return "repaired again: nothing changes".to_string();
        }
        if !again.built {
            return "repaired again: the second run built no package".to_string();
        }
        let fixups: Vec<String> = again
            .fixups
            .iter()
            .map(|row| format!("{} ({})", row.fixup, row.values))
            .collect();
        let fixups = match fixups.is_empty() {
            true => "none".to_string(),
            false => fixups.join(", "),
        };
        format!(
            "repaired again: {} chunks differ; values the fixups changed again: {fixups}",
            again.chunks_differing
        )
    }

    /// The report's parts but the diagnostics, which are added when the report is written:
    /// beside the package before the write, and to `--report` after it.
    fn report(
        env: &Env,
        reached: &Reached,
        package: Option<&Package>,
        speller: &Speller<'_>,
    ) -> ReportBuilder {
        let mut report = Report::builder().env(env);
        if let Some(m) = reached.module() {
            report = report.module(m);
        }
        if let Reached::Fixed(fixed) = reached {
            report = report.dating(&fixed.dating, &fixed.module).changes(
                &fixed.changes,
                &fixed.module,
                speller,
            );
        }
        if let Some(package) = package {
            report = report.package(package);
        }
        report
    }

    /// Writes the project directory and, when asked, the `.modpkg`, each with the report's
    /// text beside it.
    fn write(
        &self,
        package: &Package,
        out: &Utf8PathBuf,
        report: &str,
        log: &mut Diagnostics,
    ) -> Result<()> {
        let project = ProjectDir::new(out);
        let packed = Modpkg::new(out);
        let mut writers: Vec<&dyn PackageWriter> = vec![&project];
        if self.modpkg {
            writers.push(&packed);
        }
        for writer in writers {
            package
                .write(writer, report, &mut log.at(Stage::Write))
                .with_context(|| format!("writing the {} in {out}", writer.kind().as_str()))?;
        }
        Ok(())
    }
}
