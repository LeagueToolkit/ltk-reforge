//! What the commands that run a mod share: the inputs a run is judged against, the choices a
//! command line makes for a run, the write, and the diagnostics as a terminal shows them.

use std::process::ExitCode;

use anyhow::{Context, Result, bail};
use camino::{Utf8Path, Utf8PathBuf};
use clap::Args;
use fs_err as fs;
use reforge::archive::Caps;
use reforge::diag::{Action, Cancel, DiagnosticPolicy, Filter, Render, Sink, Stage};
use reforge::env::Env;
use reforge::game::{Game, Install, MimirAbsent, Names, Schema, mimir_cache_dir};
use reforge::history;
use reforge::model::ContentId;
use reforge::package::{CheckSelection, CheckSet, Modpkg, PackageWriter, ProjectDir};
use reforge::snapshot::RawCache;
use reforge::{Run, RunError};
use reforge_census::GitRepo;

/// The exit code of a run that a fatal diagnostic ended. A fault ends with 1.
pub const HALTED: u8 = 2;

/// The exit code of a run whose write the caller's policy refused.
pub const BLOCKED: u8 = 3;

/// The exit code of a run the terminal's interrupt cancelled (#ADR-0097).
pub const CANCELLED: u8 = 4;

/// A cancel the terminal's interrupt sets. The first interrupt lets the stage under way end
/// its step, and the run ends there with its report; a second ends the process at once.
pub fn interrupt() -> Result<Cancel> {
    let cancel = Cancel::new();
    let set = cancel.clone();
    ctrlc::set_handler(move || {
        if set.is_set() {
            std::process::exit(i32::from(CANCELLED));
        }
        errln!("interrupted: the run ends after the step under way");
        set.cancel();
    })
    .context("setting the handler of the terminal's interrupt")?;
    Ok(cancel)
}

/// The inputs a run is judged against, as a command line names them.
#[derive(Args)]
pub struct Inputs {
    /// The game install the mod is judged against.
    #[arg(long)]
    game: Utf8PathBuf,
    /// The history index. Without it the mod is judged against the install alone.
    #[arg(long)]
    index: Option<Utf8PathBuf>,
    /// The census history, to compare its head with the index's.
    #[arg(long)]
    repo: Option<Utf8PathBuf>,
    /// The hash cache's directory (default: the one the League Toolkit programs share on this
    /// machine).
    #[arg(long)]
    mimir: Option<Utf8PathBuf>,
    /// Read no hash cache.
    #[arg(long)]
    no_mimir: bool,
    /// A directory of CommunityDragon text tables. Repeatable, read in the order given.
    #[arg(long = "text-tables")]
    text_tables: Vec<Utf8PathBuf>,
    /// Where the name-hash index of the hash cache's paths is kept. Built there when absent.
    #[arg(long)]
    names_index: Option<Utf8PathBuf>,
}

/// Where a run keeps what it reads of an archive, as a command line names it (#ADR-0095).
#[derive(Args)]
pub struct Cache {
    /// A directory to keep each archive's files in, by their content, with what the read of
    /// the archive found: a later run of the same archive reads them from there and inflates
    /// nothing. Made when absent; any number of runs share one. Without it, a run holds the
    /// files in memory alone.
    #[arg(long)]
    store: Option<Utf8PathBuf>,
}

impl Cache {
    /// The run, reading its archive through the cache when the command line names one. With
    /// a store, an archive named by its SHA-256 starts the run from the raw snapshot the store
    /// holds of it, read under `caps` (#ADR-0094); a store that holds none is refused.
    pub fn over<'a>(&self, run: Run<'a>, archive: &Utf8Path, caps: &Caps) -> Result<Run<'a>> {
        let hash = by_hash(archive);
        let Some(dir) = &self.store else {
            if hash.is_some() {
                bail!("{archive} is a SHA-256: an archive is named by it only with --store");
            }
            return Ok(run);
        };
        let cache = RawCache::new(dir.clone());
        let run = run.in_cache(cache.clone());
        let Some(sha256) = hash else {
            return Ok(run);
        };
        let Some(raw) = cache.get(sha256, caps) else {
            bail!("the store {dir} holds no archive of SHA-256 {archive}");
        };
        run.resume(raw)
            .with_context(|| format!("starting from the store's snapshot of {archive}"))
    }
}

/// The SHA-256 an archive argument names in place of a path: 64 hex digits, and no file of
/// that name.
fn by_hash(archive: &Utf8Path) -> Option<ContentId> {
    ContentId::parse(archive.as_str()).filter(|_| !archive.is_file())
}

/// Which meta schema database a run reads.
#[derive(Debug, Clone, Copy)]
pub enum SchemaInput<'a> {
    /// No database: the run reads no value's type.
    Unread,
    /// The database at a path, else the one the League Toolkit programs share on this machine.
    Read(Option<&'a Utf8Path>),
}

impl Inputs {
    /// Opens the inputs. Each raises what it found about itself.
    pub fn env(&self, schema: SchemaInput<'_>, sink: &mut Sink<'_>) -> Result<Env> {
        let install = Install::open(&self.game)?;
        install.report(sink);
        let build = install.build().cloned();

        let mut names = Names::builder();
        let mimir = match (&self.mimir, self.no_mimir) {
            (_, true) => None,
            (Some(dir), false) => Some(dir.clone()),
            (None, false) => {
                let usual = mimir_cache_dir();
                if usual.is_none() {
                    sink.raise(MimirAbsent::no_directory());
                }
                usual
            }
        };
        if let Some(dir) = mimir {
            names = names.mimir(dir);
        }
        for dir in &self.text_tables {
            names = names.text_tables(dir);
        }
        if let Some(dir) = &self.names_index {
            names = names.name_hash_index(dir);
        }
        let names = names.build().context("opening the name tables")?;
        names.report(sink);

        let history = history::open(self.index.as_deref(), sink);
        if let Some(stamp) = history.stamp() {
            let head = match &self.repo {
                Some(repo) => Some(GitRepo::open(repo)?.head()),
                None => None,
            };
            stamp.check(head.as_deref(), build.as_ref(), sink);
        }
        let schema = match schema {
            SchemaInput::Unread => None,
            SchemaInput::Read(path) => {
                let path = path.map(Utf8Path::to_path_buf);
                let path = path.or_else(Schema::shared_copy);
                let schema = Schema::open_reported(path.as_deref(), sink);
                if let Some(schema) = &schema {
                    schema.report(build.as_ref(), sink);
                }
                schema
            }
        };
        let env = Env::builder().install(install).history(history);
        Ok(env.names(names).schema(schema).build()?)
    }
}

/// Reads the archive file into a run, unless the run started from its raw snapshot, and
/// takes the run through its stages up to `last`. A fatal diagnostic ends the run early and
/// is no error.
pub fn through(run: &mut Run<'_>, archive: &Utf8Path, last: Stage) -> Result<()> {
    if !run.ran(Stage::Read) {
        match run.read_file(archive) {
            Ok(_) | Err(RunError::Halted(_)) => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(run.through(last)?)
}

/// The checks a command line leaves on. An id that names no check is refused.
pub fn checks(off: &[String]) -> Result<CheckSet> {
    let mut selection = CheckSelection::standard();
    for id in off {
        selection = selection.without(id.as_str());
    }
    if let Some(unknown) = selection.unknown().first() {
        let known: Vec<String> = CheckSet::catalog()
            .iter()
            .map(|info| info.id.to_string())
            .collect();
        bail!("{unknown} is no check: {}", known.join(", "));
    }
    Ok(CheckSet::from(&selection))
}

/// The policy a command line gives: nothing blocks unless a filter says so.
pub fn blocking(filters: &[String]) -> Result<DiagnosticPolicy> {
    let mut policy = DiagnosticPolicy::default();
    for text in filters {
        let filter = Filter::parse(text).with_context(|| format!("the filter {text:?}"))?;
        policy = policy.with(filter, Action::Block);
    }
    Ok(policy)
}

/// Writes a run's package as a project directory in `out` and, when asked, as a `.modpkg`,
/// each with the report beside it. Says whether the policy refused the write.
pub fn write(
    run: &mut Run<'_>,
    out: &Utf8Path,
    modpkg: bool,
    policy: &DiagnosticPolicy,
) -> Result<bool> {
    // A cancelled run writes nothing, and its report says where it ended.
    if run.cancelled_at().is_some() {
        return Ok(false);
    }
    let project = ProjectDir::new(out);
    let packed = Modpkg::new(out);
    let mut writers: Vec<&dyn PackageWriter> = vec![&project];
    if modpkg {
        writers.push(&packed);
    }
    match run.write(&writers, policy) {
        Ok(()) => Ok(false),
        Err(RunError::Refused(_)) => Ok(true),
        Err(error) => Err(error).with_context(|| format!("writing in {out}")),
    }
}

/// Writes a run's report, as JSON, to a file.
pub fn save_report(run: &Run<'_>, path: &Utf8Path) -> Result<()> {
    Ok(fs::write(path, run.report().to_json() + "\n")?)
}

/// The exit code of a run: halted, cancelled, refused its write, or done.
pub fn exit(run: &Run<'_>, refused: bool) -> ExitCode {
    let halted = run.halted().is_some();
    ExitCode::from(code(halted, run.cancelled_at().is_some(), refused))
}

/// The exit code of a run that ended as these say. A fatal diagnostic leaves no stage for a
/// cancel to end, and a cancelled run writes nothing for a policy to refuse.
fn code(halted: bool, cancelled: bool, refused: bool) -> u8 {
    match (halted, cancelled, refused) {
        (true, _, _) => HALTED,
        (false, true, _) => CANCELLED,
        (false, false, true) => BLOCKED,
        (false, false, false) => 0,
    }
}

/// Prints every diagnostic of a run that a policy does not hide, one per line.
pub fn print(run: &Run<'_>, policy: &DiagnosticPolicy) {
    let speller = run.speller();
    let cx = &Render {
        names: &speller,
        module: run.module(),
    };
    for diagnostic in run.log().iter().filter(|d| !policy.hides(d)) {
        // The fixups and the checks share their codes: the one that raised a line is named
        // at its end.
        let by = diagnostic.by.as_deref();
        let by = by.map_or(String::new(), |id| format!(" [{id}]"));
        outln!(
            "{}[{}] {}: {}{by}",
            diagnostic.severity(),
            diagnostic.code,
            cx.subject_name(&diagnostic.subject),
            diagnostic.message(cx)
        );
    }
    if let Some(stage) = run.cancelled_at() {
        errln!("cancelled in {stage}: no package is written");
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    #[test]
    fn an_archive_argument_of_64_hex_digits_is_a_sha256() {
        let hex = "9F86D081884C7D659A2FEAA0C55AD015A3BF4F1B2B0B822CD15D6C15B0F00A08";
        let named = by_hash(Utf8Path::new(hex)).expect("a SHA-256");
        assert_eq!(named.hex(), hex.to_ascii_lowercase());
        assert_eq!(by_hash(Utf8Path::new("mod.fantome")), None);
        assert_eq!(by_hash(Utf8Path::new(&hex[1..])), None, "63 digits");
    }

    #[test]
    fn a_cancelled_run_exits_with_its_own_code_beside_the_halted_one() {
        assert_eq!(code(false, false, false), 0);
        assert_eq!(code(true, false, false), HALTED);
        assert_eq!(code(false, true, false), CANCELLED);
        assert_eq!(code(false, false, true), BLOCKED);
        let codes = [HALTED, BLOCKED, CANCELLED];
        assert!(codes.iter().all(|&code| code > 1), "1 is a fault's");
        assert_eq!(codes.iter().collect::<BTreeSet<_>>().len(), codes.len());
    }
}
