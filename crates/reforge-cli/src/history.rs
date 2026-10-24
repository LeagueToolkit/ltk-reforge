//! `reforge history`: build the index, check it, probe a game file, read an entry.

use std::time::Instant;

use anyhow::{Context, Result, bail};
use camino::Utf8PathBuf;
use clap::{Args, Subcommand};
use reforge::diag::{Diagnostics, Render, Stage};
use reforge::game::{Game, Install};
use reforge::history::{EventsBankHit, Families, History, SqliteHistory, Stamp};
use reforge::model::{
    BinHash, ClipKey, ContentId, FileKey, NormKey, ObjectKey, WadHash, bin, name_hash, path_hash,
};
use reforge_census::{
    DeltaStep, GitRepo, Indexer, Unkeyed, apply_delta, check_objects, write_delta,
};
use reforge_wwise::Bank;

/// Where the index is when no option says.
const INDEX_DIR: &str = "data/history-index";

#[derive(Subcommand)]
pub enum HistoryCommand {
    /// Walk the census history and write its index.
    Index(Index),
    /// Print the index's stamp and whether it matches the repository and the install.
    Status(Status),
    /// Hash a game file of the install and look it up in the index, with a bin's objects, a
    /// bank's clips, an events bank's header and objects, or an animation clip's joints: a
    /// check of the index against real data.
    Probe(Probe),
    /// Print a bin entry's text at a build.
    ShowEntry(ShowEntry),
    /// Check the index's object bytes: which texts have their object, and whether each
    /// object's digest is an object key of the index under its text's class.
    CheckObjects(CheckObjects),
    /// Write the delta that takes one index to another taken forward from it, for publishing
    /// beside the first.
    Delta(Delta),
    /// Apply a published delta to the index it starts from, into a new folder.
    Apply(Apply),
}

impl HistoryCommand {
    pub fn run(&self) -> Result<()> {
        match self {
            HistoryCommand::Index(command) => command.run(),
            HistoryCommand::Status(command) => command.run(),
            HistoryCommand::Probe(command) => command.run(),
            HistoryCommand::ShowEntry(command) => command.run(),
            HistoryCommand::CheckObjects(command) => command.run(),
            HistoryCommand::Delta(command) => command.run(),
            HistoryCommand::Apply(command) => command.run(),
        }
    }
}

#[derive(Args)]
pub struct Delta {
    /// The index the delta starts from: the published snapshot.
    #[arg(long)]
    base: Utf8PathBuf,
    /// The index taken forward from it, which the delta makes.
    #[arg(long, default_value = INDEX_DIR)]
    index: Utf8PathBuf,
    /// Where the delta is written.
    #[arg(long)]
    out: Utf8PathBuf,
}

impl Delta {
    fn run(&self) -> Result<()> {
        let started = Instant::now();
        let note = write_delta(&self.out, &self.base, &self.index)?;
        let size = fs_err::metadata(&self.out)?.len();
        outln!(
            "delta at {}: {} builds, {} to {}, {} MB for a database of {} MB ({:.1?})",
            self.out,
            note.builds,
            short(&note.base_head),
            short(&note.head),
            size >> 20,
            note.size >> 20,
            started.elapsed()
        );
        Ok(())
    }
}

#[derive(Args)]
pub struct Apply {
    /// The delta.
    #[arg(long)]
    delta: Utf8PathBuf,
    /// The index the delta starts from.
    #[arg(long)]
    base: Utf8PathBuf,
    /// Where the index the delta makes is written.
    #[arg(long)]
    out: Utf8PathBuf,
}

impl Apply {
    fn run(&self) -> Result<()> {
        let started = Instant::now();
        let mut shown = 0;
        let stamp = apply_delta(&self.delta, &self.base, &self.out, &mut |step| match step {
            DeltaStep::Write { done, total } => {
                // A line per 256 MB.
                if done >> 28 != shown || done == total {
                    shown = done >> 28;
                    errln!("written {} of {} MB", done >> 20, total >> 20);
                }
            }
            DeltaStep::Check => errln!("checking the digest"),
        })?;
        outln!(
            "index at {}: {} builds up to {} (head {}) in {:.1?}",
            self.out,
            stamp.builds,
            stamp.newest_build,
            short(&stamp.head),
            started.elapsed()
        );
        print_rows(&stamp);
        Ok(())
    }
}

#[derive(Args)]
pub struct Index {
    /// The census history, a clone bare or not: its `history-v2` branch.
    #[arg(long)]
    repo: Utf8PathBuf,
    /// The last commit or build to index (default: the branch's tip).
    #[arg(long)]
    until: Option<String>,
    /// Add the builds after the index's head to it, and do not build it whole.
    #[arg(long)]
    append: bool,
    /// Where the index is written.
    #[arg(long, default_value = INDEX_DIR)]
    out: Utf8PathBuf,
    /// Which families to index: files, objects, clips, rigs, textures.
    #[arg(long, default_value = "files,objects,clips,rigs,textures")]
    families: String,
}

impl Index {
    fn run(&self) -> Result<()> {
        let families = Families::parse(&self.families)?;
        let mut repo = GitRepo::open(&self.repo)?;
        if let Some(until) = &self.until {
            repo.stop_at(until)?;
        }
        let started = Instant::now();
        let every = self.append;
        let mut indexer = Indexer::new(&repo, self.repo.as_str()).on_progress(move |p| {
            if every || p.done == 1 || p.done == p.of || p.done % 25 == 0 {
                errln!(
                    "[{}/{}] {} {} ({:.0?})",
                    p.done,
                    p.of,
                    p.build.build,
                    p.build.date,
                    started.elapsed()
                );
            }
        });
        let stamp = if self.append {
            indexer.append(&self.out)?
        } else {
            indexer.build(&self.out, families)?
        };
        errln!("written in {:.1?}", started.elapsed());
        outln!(
            "index at {}: {} builds up to {} (head {})",
            self.out,
            stamp.builds,
            stamp.newest_build,
            short(&stamp.head)
        );
        print_rows(&stamp);
        Ok(())
    }
}

/// A commit id cut to twelve digits.
fn short(commit: &str) -> &str {
    commit.get(..12).unwrap_or(commit)
}

fn print_rows(stamp: &Stamp) {
    let rows: Vec<String> = stamp
        .rows
        .iter()
        .map(|(table, count)| format!("{table}={count}"))
        .collect();
    outln!("rows     {}", rows.join(" "));
}

#[derive(Args)]
pub struct Status {
    /// The census history, to compare its head with the index's.
    #[arg(long)]
    repo: Option<Utf8PathBuf>,
    /// The game install, to compare its build with the index's newest.
    #[arg(long)]
    game: Option<Utf8PathBuf>,
    /// The index.
    #[arg(long, default_value = INDEX_DIR)]
    index: Utf8PathBuf,
}

impl Status {
    fn run(&self) -> Result<()> {
        let stamp = Stamp::read(&self.index)?;
        let families: Vec<String> = stamp.families.iter().map(|f| f.to_string()).collect();
        outln!("index    {}", self.index);
        outln!("format   {}", stamp.format);
        outln!("repo     {}", stamp.repo);
        outln!("head     {}", stamp.head);
        outln!("builds   {} up to {}", stamp.builds, stamp.newest_build);
        outln!("families {}", families.join(","));
        print_rows(&stamp);
        if stamp.unparsed_texts > 0 {
            outln!("unparsed {} entry texts name nothing", stamp.unparsed_texts);
        }
        if stamp.objectless_texts > 0 {
            outln!(
                "objectless {} entry texts hold no object or several",
                stamp.objectless_texts
            );
        }
        let codec = &stamp.objects;
        outln!(
            "objects  zstd {} level {}, a keyframe every {} versions, dictionaries up to {} bytes",
            codec.zstd,
            codec.level,
            codec.keyframe_every,
            codec.dict_cap
        );

        let head = match &self.repo {
            Some(repo) => Some(GitRepo::open(repo)?.head()),
            None => None,
        };
        let install = match &self.game {
            Some(game) => Some(Install::open(game)?),
            None => None,
        };
        let mut log = Diagnostics::new();
        stamp.check(
            head.as_deref(),
            install.as_ref().and_then(|install| install.build()),
            &mut log.at(Stage::Read),
        );
        for diagnostic in log.iter() {
            outln!(
                "{}[{}] {}",
                diagnostic.severity(),
                diagnostic.code,
                diagnostic.message(&Render::plain())
            );
        }
        Ok(())
    }
}

#[derive(Args)]
pub struct Probe {
    /// The file's path in the game, `data/characters/lux/skins/skin0.bin`.
    path: String,
    /// The game install.
    #[arg(long)]
    game: Utf8PathBuf,
    /// The index.
    #[arg(long, default_value = INDEX_DIR)]
    index: Utf8PathBuf,
    /// Fail when the file, one of its objects or one of its clips is not in the history.
    #[arg(long)]
    strict: bool,
}

impl Probe {
    fn run(&self) -> Result<()> {
        let history = SqliteHistory::open(&self.index)?;
        let install = Install::open(&self.game)?;
        let hash = path_hash(&self.path);
        let bytes = install
            .read(hash)?
            .with_context(|| format!("the install has no {}", self.path))?;
        let content = ContentId::of(&bytes);
        outln!(
            "{}: {} bytes, sha256 {}",
            self.path,
            bytes.len(),
            content.hex()
        );

        let mut missing = 0usize;
        let key = FileKey::from_sha256(&content.0);
        Self::joints(&history, key)?;
        match history.file(key)? {
            Some(hit) => outln!(
                "in the history: {} (first under {:016x}{})",
                history.span(hit.life),
                hit.path,
                if hit.path == hash { ", this path" } else { "" }
            ),
            None => {
                missing += 1;
                outln!("not in the history");
            }
        }
        if bytes.starts_with(b"PROP") {
            missing += self.objects(&history, &bytes)?;
        }
        if reforge_wwise::is_bnk(&bytes) || reforge_wwise::is_wpk(&bytes) {
            missing += self.clips(&history, &bytes, key)?;
        }
        if self.strict && missing > 0 {
            bail!("{missing} keys of {} are not in the history", self.path);
        }
        Ok(())
    }

    /// Prints how many of a bin's objects the history holds, and returns how many it lacks.
    fn objects(&self, history: &dyn History, bytes: &[u8]) -> Result<usize> {
        let parsed = bin::parse(bytes).with_context(|| format!("parsing {}", self.path))?;
        let (mut found, mut by_norm) = (0usize, 0usize);
        for object in &parsed.objects {
            let key = ObjectKey::from_sha256(&object.raw.0);
            found += usize::from(history.object(key)?.is_some());
            let norm = NormKey::from_sha256(&object.body.0);
            by_norm += usize::from(!history.objects_by_norm(norm)?.is_empty());
        }
        let of = parsed.objects.len();
        outln!("objects in the history: {found} of {of}, by norm key {by_norm}");
        Ok((of - found) + (of - by_norm))
    }

    /// Prints how many of a bank's clips the history holds, and returns how many it lacks.
    fn clips(&self, history: &dyn History, bytes: &[u8], key: FileKey) -> Result<usize> {
        let bank = Bank::read(bytes).with_context(|| format!("reading {}", self.path))?;
        let mut found = 0usize;
        for wem in &bank.media {
            let key = ClipKey::from_sha256(&ContentId::of(&wem.bytes).0);
            found += usize::from(history.clip(key)?.is_some());
        }
        let of = bank.media.len();
        outln!("clips in the history: {found} of {of}");
        let mut missing = of - found;
        if bank.has_hirc() {
            missing += self.events_bank(history, &bank, key)?;
        }
        Ok(missing)
    }

    /// Prints whether the history holds the header and objects of the events bank of these
    /// bytes at its path, and which WADs held it, and returns one when it does not.
    fn events_bank(&self, history: &dyn History, bank: &Bank, key: FileKey) -> Result<usize> {
        let held = history.events_banks(path_hash(&self.path))?;
        let same = |hit: &&EventsBankHit| {
            hit.key == key && hit.bank.header == bank.header && hit.bank.objects == bank.objects
        };
        let of = bank.objects.len();
        let found = held.iter().find(same);
        match found {
            Some(hit) => {
                let mut wads: Vec<&str> = hit.placed.iter().map(|p| p.wad.as_str()).collect();
                wads.dedup();
                outln!(
                    "header and {of} objects in the history: {}, in {}",
                    history.span(hit.life),
                    wads.join(", ")
                );
            }
            None => outln!(
                "header and {of} objects not in the history ({} other banks at this path)",
                held.len()
            ),
        }
        Ok(usize::from(found.is_none()))
    }

    /// Prints the joints the history holds for an animation clip's bytes.
    fn joints(history: &dyn History, key: FileKey) -> Result<()> {
        if let Some(joints) = history.joints_of(key)? {
            outln!("joints in the history: {}", joints.len());
        }
        Ok(())
    }
}

#[derive(Args)]
pub struct ShowEntry {
    /// The entry's name, or its hash as `0x` and eight hex digits.
    entry: String,
    /// The bin that holds it: its path, or its path hash as `0x` and sixteen hex digits.
    #[arg(long)]
    bin: String,
    /// The census history.
    #[arg(long)]
    repo: Utf8PathBuf,
    /// A commit id or prefix, or a build version (default: the head).
    #[arg(long)]
    at: Option<String>,
    /// The index of the history, which names the text's blob at the build so that the build's
    /// tree is not searched for it.
    #[arg(long)]
    index: Option<Utf8PathBuf>,
}

impl ShowEntry {
    fn run(&self) -> Result<()> {
        let repo = GitRepo::open(&self.repo)?;
        let entry = self.entry_hash()?;
        let bin = self.bin_hash()?;
        let commit = match &self.at {
            Some(at) => repo.resolve(at)?,
            None => repo.head(),
        };
        let text = match &self.index {
            Some(index) => {
                let history = SqliteHistory::open(index)?;
                let build = history.builds().iter().find(|row| row.commit == commit);
                let at = build
                    .with_context(|| format!("the index at {index} holds no build {commit}"))?
                    .at;
                repo.entry_text_at(&history, bin, entry, at)?
            }
            None => repo.entry_text(bin, entry, &commit)?,
        };
        let text =
            text.with_context(|| format!("no entry {entry:08x} in bin {bin:016x} at {commit}"))?;
        out!("{text}");
        Ok(())
    }

    fn entry_hash(&self) -> Result<BinHash> {
        match self.entry.strip_prefix("0x") {
            Some(hex) => u32::from_str_radix(hex, 16)
                .map(BinHash)
                .with_context(|| format!("{} is not a hash", self.entry)),
            None => Ok(name_hash(&self.entry)),
        }
    }

    fn bin_hash(&self) -> Result<WadHash> {
        match self.bin.strip_prefix("0x") {
            Some(hex) => u64::from_str_radix(hex, 16)
                .map(WadHash)
                .with_context(|| format!("{} is not a hash", self.bin)),
            None => Ok(path_hash(&self.bin)),
        }
    }
}

#[derive(Args)]
pub struct CheckObjects {
    /// The index.
    #[arg(long, default_value = INDEX_DIR)]
    index: Utf8PathBuf,
    /// Where to write each text whose object's digest is no object key, one per line: blob,
    /// class, digest, bin, entry, first build, whether that build's bins are legacy, and
    /// whether the object holds a kind legacy numbering writes as another byte.
    #[arg(long)]
    unkeyed: Option<Utf8PathBuf>,
}

impl CheckObjects {
    fn run(&self) -> Result<()> {
        let started = Instant::now();
        let history = SqliteHistory::open(&self.index)?;
        let check = check_objects(&self.index)?;
        let renumbered = |u: &&Unkeyed| u.legacy && u.renumbered;
        let explained = check.unkeyed.iter().filter(renumbered).count();
        outln!(
            "texts    {} with their object, {} of {} without",
            check.stored,
            check.texts - check.stored,
            check.texts
        );
        outln!("orphans  {} objects of no text", check.orphans);
        outln!("objects  {} rows of object bytes", check.objects);
        outln!(
            "keyed    {} of {} objects' digests are object keys; {} are not",
            check.keyed,
            check.stored,
            check.unkeyed.len()
        );
        outln!(
            "unkeyed  {explained} first held at a build of legacy bins and holding a kind legacy \
             numbering writes otherwise, {} not",
            check.unkeyed.len() - explained
        );
        if let Some(path) = &self.unkeyed {
            let hex =
                |bytes: &[u8]| -> String { bytes.iter().map(|b| format!("{b:02x}")).collect() };
            let lines: String = check
                .unkeyed
                .iter()
                .map(|u| {
                    let first = history
                        .version(u.first)
                        .map_or_else(|| format!("#{}", u.first), ToString::to_string);
                    format!(
                        "{}\t{:08x}\t{}\t{:016x}\t{:08x}\t{first}\t{}\t{}\n",
                        hex(&u.blob),
                        u.class.0,
                        u.key,
                        u.bin.0,
                        u.entry.0,
                        u8::from(u.legacy),
                        u8::from(u.renumbered)
                    )
                })
                .collect();
            fs_err::write(path, lines)?;
            outln!("unkeyed  written to {path}");
        }
        errln!("checked in {:.1?}", started.elapsed());
        Ok(())
    }
}
