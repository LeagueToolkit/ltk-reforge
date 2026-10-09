//! The writer of the object bytes (`reforge::history::objects`, #ADR-0078).
//!
//! The walk keeps the object of each text it reads first, packed at zstd's fastest level, until
//! every text is in. A whole build then orders each entry's versions, trains the keyframes'
//! dictionaries and compresses on every core. An append compresses each version a build adds
//! against its entry's keyframe, which it reads from the tables, with the dictionaries the whole
//! build trained.
//!
//! An entry's versions are its distinct objects, ordered by the build each first appeared at and
//! then by the SHA-256 of its bytes. Every `keyframe_every`th from the first is a keyframe. The
//! same order and the same blocks come out of a whole build and of an append, so the two hold the
//! same bytes; their frames differ only where their dictionaries do.
//!
//! A whole build holds millions of objects. They are kept in large chunks, read through one zstd
//! context per thread, and written a batch at a time: an allocation per object made the build's
//! write phase take hours on a heap the walk had filled (2026-10-08).

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::ops::Range;

use rayon::prelude::*;
use reforge::history::objects::{Decoder, FrameError, MAX_OBJECT, ObjectCodec, stated_size};
use reforge::history::{BlobId, Position};
use reforge::model::BinHash;
use rusqlite::{OptionalExtension, Transaction, params};
use sha2::{Digest, Sha256};
use zstd::zstd_safe::{self, CCtx, CDict, DCtx};

use crate::error::CensusError;
use crate::format::TextObject;

/// The level an object waits at for the whole build: the fastest, as it is unpacked once.
const PACK_LEVEL: i32 = 1;

/// The bytes of one chunk of the store's packed objects.
const CHUNK: usize = 64 << 20;

/// The smallest dictionary trained. A class whose keyframes hold fewer bytes than
/// `SAMPLES_PER_DICT_BYTE` of these shares the general dictionary.
const MIN_DICT: usize = 16 << 10;

/// Training samples per byte of dictionary, as zstd advises.
const SAMPLES_PER_DICT_BYTE: usize = 100;

/// Blocks compressed at a time before their frames are written.
const BLOCKS_PER_BATCH: usize = 1 << 14;

thread_local! {
    /// This thread's context and buffer for packing objects.
    static PACKER: RefCell<(CCtx<'static>, Vec<u8>)> = RefCell::new((CCtx::create(), Vec::new()));
    /// This thread's context for unpacking them.
    static UNPACKER: RefCell<DCtx<'static>> = RefCell::new(DCtx::create());
}

/// A text's object, as the core that parsed it packed it.
pub(crate) struct Packed {
    /// The entry the text is of.
    entry: BinHash,
    class: BinHash,
    /// The build the text first appeared at.
    first: Position,
    /// The SHA-256 of the bytes.
    digest: [u8; 32],
    len: u32,
    packed: Vec<u8>,
}

/// What a text read for the first time holds, made on the core that parsed it.
pub(crate) enum Fresh {
    /// The text's one object.
    One(Packed),
    /// The text holds no object or several, or one past `MAX_OBJECT`, which no frame of the
    /// index decodes.
    Objectless,
}

/// The keyframe interval the codec names, as a count.
fn keyframe_every(codec: &ObjectCodec) -> usize {
    usize::try_from(codec.keyframe_every)
        .expect("a keyframe interval fits usize")
        .max(1)
}

impl Fresh {
    /// A text's objects, of the entry `entry`, first read at the build at `at`.
    pub(crate) fn of(entry: BinHash, at: Position, objects: Vec<TextObject>) -> Self {
        let mut objects = objects.into_iter();
        let (Some(object), None) = (objects.next(), objects.next()) else {
            return Fresh::Objectless;
        };
        let len = object.bytes.len();
        if len > MAX_OBJECT {
            return Fresh::Objectless;
        }
        let len32 = u32::try_from(len).expect("an object within MAX_OBJECT fits u32");
        let digest: [u8; 32] = Sha256::digest(&object.bytes).into();
        let packed = PACKER.with(|cell| {
            let (cctx, scratch) = &mut *cell.borrow_mut();
            scratch.clear();
            scratch.reserve(zstd_safe::compress_bound(len));
            let done = cctx.compress(scratch, &object.bytes, PACK_LEVEL);
            done.unwrap_or_else(|code| {
                let reason = zstd_safe::get_error_name(code);
                panic!(
                    "packing an object of {len} bytes of entry {:08x}: {reason}",
                    entry.0
                )
            });
            scratch.clone()
        });
        Fresh::One(Packed {
            entry,
            class: object.class,
            first: at,
            digest,
            len: len32,
            packed,
        })
    }
}

/// A text's object as the store keeps it: where its packed bytes are.
#[derive(Clone, Copy)]
pub(crate) struct Pending {
    /// The entry the text is of.
    entry: BinHash,
    class: BinHash,
    /// The build the text first appeared at.
    first: Position,
    /// The SHA-256 of the bytes.
    digest: [u8; 32],
    len: u32,
    chunk: u32,
    start: u32,
    size: u32,
}

/// The objects of the texts a walk read, each kept from the first build its text appeared at.
#[derive(Default)]
pub(crate) struct Store {
    /// The packed objects, one after another.
    chunks: Vec<Vec<u8>>,
    pending: HashMap<BlobId, Pending>,
    /// Texts that hold no object or several, by blob, with the entry they are of.
    objectless: HashMap<BlobId, BinHash>,
}

fn index32(n: usize, what: &str) -> u32 {
    u32::try_from(n).unwrap_or_else(|_| panic!("{what} {n} is past the store's 32-bit index"))
}

impl Store {
    /// Keeps what a text read for the first time holds. A text read again, after it left the
    /// tree and came back, keeps what it held first.
    pub(crate) fn add(&mut self, blob: BlobId, entry: BinHash, fresh: Fresh) {
        if self.pending.contains_key(&blob) || self.objectless.contains_key(&blob) {
            return;
        }
        match fresh {
            Fresh::One(packed) => {
                let fits = self
                    .chunks
                    .last()
                    .is_some_and(|c| c.capacity() - c.len() >= packed.packed.len());
                if !fits {
                    self.chunks
                        .push(Vec::with_capacity(CHUNK.max(packed.packed.len())));
                }
                let chunk = index32(self.chunks.len() - 1, "chunk");
                let held = self.chunks.last_mut().expect("a chunk was pushed");
                let start = index32(held.len(), "offset");
                held.extend_from_slice(&packed.packed);
                let pending = Pending {
                    entry: packed.entry,
                    class: packed.class,
                    first: packed.first,
                    digest: packed.digest,
                    len: packed.len,
                    chunk,
                    start,
                    size: index32(packed.packed.len(), "size"),
                };
                self.pending.insert(blob, pending);
            }
            Fresh::Objectless => {
                self.objectless.insert(blob, entry);
            }
        }
    }

    /// How many texts hold no object or several.
    pub(crate) fn objectless(&self) -> usize {
        self.objectless.len()
    }

    /// The bytes of an object the store holds, into `out`.
    fn unpack(&self, p: &Pending, out: &mut Vec<u8>) {
        let chunk = &self.chunks[p.chunk as usize];
        let packed = &chunk[p.start as usize..][..p.size as usize];
        out.clear();
        out.reserve(p.len as usize);
        let done = UNPACKER.with(|d| d.borrow_mut().decompress(out, packed));
        done.unwrap_or_else(|code| {
            let reason = zstd_safe::get_error_name(code);
            panic!(
                "unpacking an object of {} bytes of entry {:08x}: {reason}",
                p.len, p.entry.0
            )
        });
    }

    /// The bytes of an object the store holds.
    fn bytes(&self, p: &Pending) -> Vec<u8> {
        let mut out = Vec::new();
        self.unpack(p, &mut out);
        out
    }
}

/// How one frame is compressed.
enum With<'a> {
    Nothing,
    Dict(&'a CDict<'static>),
    Raw(&'a [u8]),
}

/// A frame of `src`, compressed in `scratch` and copied out at its size.
fn compress(
    cctx: &mut CCtx<'_>,
    scratch: &mut Vec<u8>,
    level: i32,
    src: &[u8],
    with: With<'_>,
) -> Vec<u8> {
    scratch.clear();
    scratch.reserve(zstd_safe::compress_bound(src.len()));
    let done = match with {
        With::Nothing => cctx.compress_using_dict(scratch, src, &[], level),
        With::Dict(cdict) => cctx.compress_using_cdict(scratch, src, cdict),
        With::Raw(dict) => cctx.compress_using_dict(scratch, src, dict, level),
    };
    done.unwrap_or_else(|code| {
        let reason = zstd_safe::get_error_name(code);
        panic!("compressing an object of {} bytes: {reason}", src.len())
    });
    scratch.clone()
}

/// One distinct object: its texts, `held[lead..lead + count]`, the first that held it leading.
struct Content {
    lead: usize,
    count: usize,
}

/// A dictionary trained, by the class it serves, none for the general one.
struct Trained {
    class: Option<BinHash>,
    dict: Vec<u8>,
}

/// The dictionary a keyframe of `class` is compressed with: its class's, or the general one.
fn dict_of(trained: &[(i64, CDict<'static>, Option<BinHash>)], class: BinHash) -> Option<usize> {
    let own = trained.iter().position(|t| t.2 == Some(class));
    own.or_else(|| trained.iter().position(|t| t.2.is_none()))
}

/// The keyframes' dictionaries: one per class whose keyframes hold enough bytes, and the general
/// one for the rest, each trained on samples taken at even steps through its keyframes.
fn train(store: &Store, keyframes: &[&Pending], cap: usize) -> Vec<Trained> {
    let mut by_class: BTreeMap<BinHash, usize> = BTreeMap::new();
    for k in keyframes {
        *by_class.entry(k.class).or_default() += k.len as usize;
    }
    let own = |class: BinHash| by_class[&class] >= MIN_DICT * SAMPLES_PER_DICT_BYTE;
    let mut groups: BTreeMap<Option<BinHash>, Vec<&Pending>> = BTreeMap::new();
    for k in keyframes {
        let group = Some(k.class).filter(|c| own(*c));
        groups.entry(group).or_default().push(k);
    }
    let groups: Vec<(Option<BinHash>, Vec<&Pending>)> = groups.into_iter().collect();
    let trained = groups.into_par_iter().filter_map(|(class, members)| {
        let bytes: usize = members.iter().map(|k| k.len as usize).sum();
        let size = (bytes / SAMPLES_PER_DICT_BYTE).clamp(MIN_DICT, cap);
        if bytes < size {
            return None;
        }
        let stride = (bytes / (size * SAMPLES_PER_DICT_BYTE)).max(1);
        let samples: Vec<Vec<u8>> = members
            .iter()
            .step_by(stride)
            .map(|k| store.bytes(k))
            .collect();
        // Too few samples or too alike: the group's keyframes are compressed with none.
        let dict = zstd::dict::from_samples(&samples, size).ok()?;
        Some(Trained { class, dict })
    });
    trained.collect()
}

/// Writes the object bytes of a whole build.
pub(crate) fn write_whole(
    tx: &Transaction<'_>,
    mut store: Store,
    codec: &ObjectCodec,
) -> Result<(), CensusError> {
    let every = keyframe_every(codec);
    let cap = usize::try_from(codec.dict_cap).unwrap_or(usize::MAX);

    // The texts of one object together, the first that held it leading.
    let mut held: Vec<(BlobId, Pending)> = std::mem::take(&mut store.pending).into_iter().collect();
    held.sort_unstable_by(|a, b| {
        let key = |(blob, p): &(BlobId, Pending)| (p.entry, p.digest, p.first, *blob);
        key(a).cmp(&key(b))
    });
    let mut contents: Vec<Content> = Vec::new();
    for (i, (_, p)) in held.iter().enumerate() {
        match contents.last_mut() {
            Some(c) if (held[c.lead].1.entry, held[c.lead].1.digest) == (p.entry, p.digest) => {
                c.count += 1;
            }
            _ => contents.push(Content { lead: i, count: 1 }),
        }
    }
    let lead = |c: &Content| &held[c.lead].1;
    // Each entry's versions in the order they appeared.
    contents.sort_unstable_by(|a, b| {
        let key = |c: &Content| (lead(c).entry, lead(c).first, lead(c).digest);
        key(a).cmp(&key(b))
    });
    // The blocks of versions behind a keyframe, each in `contents` order. An entry's first
    // version is a keyframe, so no block holds two entries.
    let mut blocks: Vec<Range<usize>> = Vec::new();
    let mut version = 0usize;
    for (i, content) in contents.iter().enumerate() {
        let same = i > 0 && lead(&contents[i - 1]).entry == lead(content).entry;
        version = if same { version + 1 } else { 0 };
        if version.is_multiple_of(every) {
            if let Some(last) = blocks.last_mut() {
                last.end = i;
            }
            blocks.push(i..contents.len());
        }
    }
    let keyframes: Vec<&Pending> = blocks.iter().map(|b| lead(&contents[b.start])).collect();

    let trained = train(&store, &keyframes, cap);
    let mut dicts = Vec::with_capacity(trained.len());
    for (id, t) in (1i64..).zip(&trained) {
        tx.execute(
            "INSERT INTO object_dicts (id, class, dict) VALUES (?1, ?2, ?3)",
            params![id, t.class.map(|c| c.0), t.dict],
        )?;
        let cdict = CDict::try_create(&t.dict, codec.level).ok_or(CensusError::ObjectDict(id))?;
        dicts.push((id, cdict, t.class));
    }

    // A batch of blocks on every core, then its frames written in block order. A row's id is
    // its place in `contents`, from 1.
    let id_of =
        |i: usize| i64::try_from(i + 1).unwrap_or_else(|_| panic!("object {i} is past i64"));
    let mut texts: Vec<(BlobId, BinHash, i64)> = Vec::with_capacity(held.len());
    let mut insert =
        tx.prepare("INSERT INTO object_bytes (id, keyframe, dict, frame) VALUES (?1, ?2, ?3, ?4)")?;
    for batch in blocks.chunks(BLOCKS_PER_BATCH) {
        let frames: Vec<Vec<Vec<u8>>> = batch
            .par_iter()
            .map_init(
                || (CCtx::create(), Vec::new(), Vec::new(), Vec::new()),
                |(cctx, scratch, base, bytes), block| {
                    let keyframe = lead(&contents[block.start]);
                    store.unpack(keyframe, base);
                    let dict = dict_of(&dicts, keyframe.class).map(|d| &dicts[d].1);
                    let with = dict.map_or(With::Nothing, With::Dict);
                    let mut out = vec![compress(cctx, scratch, codec.level, base, with)];
                    for content in &contents[block.start + 1..block.end] {
                        store.unpack(lead(content), bytes);
                        let frame = compress(cctx, scratch, codec.level, bytes, With::Raw(base));
                        out.push(frame);
                    }
                    out
                },
            )
            .collect();
        for (block, frames) in batch.iter().zip(frames) {
            let keyframe = id_of(block.start);
            let dict = dict_of(&dicts, lead(&contents[block.start]).class).map(|d| dicts[d].0);
            for (i, frame) in block.clone().zip(frames) {
                let id = id_of(i);
                let dict = if i == block.start { dict } else { None };
                insert.execute(params![id, keyframe, dict, frame])?;
                let content = &contents[i];
                let texts_of = &held[content.lead..content.lead + content.count];
                texts.extend(texts_of.iter().map(|(blob, p)| (*blob, p.class, id)));
            }
        }
    }
    texts.sort_unstable_by_key(|t| t.0);
    let mut insert =
        tx.prepare("INSERT INTO text_bytes (blob, class, object) VALUES (?1, ?2, ?3)")?;
    for (blob, class, id) in texts {
        insert.execute(params![&blob[..], class.0, id])?;
    }
    Ok(())
}

/// A row of `object_bytes` an entry holds.
struct Held {
    id: i64,
    keyframe: i64,
    dict: Option<i64>,
    frame: Vec<u8>,
}

/// Writes the object bytes of the builds an append adds, with the index's dictionaries.
pub(crate) struct Appender {
    level: i32,
    every: usize,
    /// The dictionary of each class that has one, and the general one under none.
    dicts: Vec<(i64, CDict<'static>, Option<BinHash>)>,
    cctx: CCtx<'static>,
    scratch: Vec<u8>,
    decoder: Decoder,
    next: i64,
}

impl Appender {
    /// The appender of the index `tx` writes, with the codec its stamp names. The stamp keeps
    /// the codec, so the zstd that wrote the frames must be the one running.
    pub(crate) fn load(tx: &Transaction<'_>, codec: &ObjectCodec) -> Result<Self, CensusError> {
        let running = zstd_safe::version_string();
        if codec.zstd != running {
            return Err(CensusError::ObjectZstd {
                stamped: codec.zstd.clone(),
                running: running.to_string(),
            });
        }
        let mut decoder = Decoder::default();
        let mut dicts = Vec::new();
        let mut read = tx.prepare("SELECT id, class, dict FROM object_dicts ORDER BY id")?;
        let mut rows = read.query([])?;
        while let Some(row) = rows.next()? {
            let (id, class, dict): (i64, Option<u32>, Vec<u8>) =
                (row.get(0)?, row.get(1)?, row.get(2)?);
            decoder.hold(id, &dict);
            let cdict = CDict::try_create(&dict, codec.level).ok_or(CensusError::ObjectDict(id))?;
            dicts.push((id, cdict, class.map(BinHash)));
        }
        let next: i64 =
            tx.query_row("SELECT IFNULL(MAX(id), 0) + 1 FROM object_bytes", [], |r| {
                r.get(0)
            })?;
        Ok(Self {
            level: codec.level,
            every: keyframe_every(codec),
            dicts,
            cctx: CCtx::create(),
            scratch: Vec::new(),
            decoder,
            next,
        })
    }

    /// The rows of `object_bytes` the texts of an entry hold, oldest first.
    fn held(tx: &Transaction<'_>, entry: BinHash) -> Result<Vec<Held>, CensusError> {
        // The ids distinct, not the frames: a frame is large and an entry's texts share rows.
        let mut read = tx.prepare_cached(
            "SELECT o.id, o.keyframe, o.dict, o.frame FROM object_bytes o WHERE o.id IN \
             (SELECT t.object FROM entry_texts e JOIN text_bytes t ON t.blob = e.blob \
             WHERE e.entry = ?1) ORDER BY o.id",
        )?;
        let rows = read.query_map(params![entry.0], |r| {
            Ok(Held {
                id: r.get(0)?,
                keyframe: r.get(1)?,
                dict: r.get(2)?,
                frame: r.get(3)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// The bytes of a row an entry holds, its keyframe among `held`.
    fn decode(&mut self, held: &[Held], row: &Held) -> Result<Vec<u8>, CensusError> {
        let broken = |source: FrameError| CensusError::ObjectFrame {
            object: row.id,
            source,
        };
        let key = held.iter().find(|k| k.id == row.keyframe);
        let key = key.ok_or(CensusError::ObjectKeyframe {
            object: row.id,
            keyframe: row.keyframe,
        })?;
        let base = self
            .decoder
            .keyframe(key.dict, &key.frame)
            .map_err(broken)?;
        if row.id == row.keyframe {
            return Ok(base);
        }
        self.decoder.delta(&base, &row.frame).map_err(broken)
    }

    /// Writes the objects of the texts one build adds that the index lacks, and answers how many
    /// of its texts new to the index hold no object or several.
    pub(crate) fn commit(
        &mut self,
        tx: &Transaction<'_>,
        store: &Store,
        at: Position,
    ) -> Result<usize, CensusError> {
        let mut seen = tx.prepare_cached(
            "SELECT EXISTS (SELECT 1 FROM entry_texts WHERE entry = ?1 AND blob = ?2 \
             AND first < ?3)",
        )?;
        let mut objectless = 0;
        for (blob, entry) in &store.objectless {
            let before: bool = seen.query_row(params![entry.0, &blob[..], at.0], |r| r.get(0))?;
            objectless += usize::from(!before);
        }
        let mut stored = tx.prepare_cached("SELECT 1 FROM text_bytes WHERE blob = ?1")?;
        let mut fresh: Vec<(BlobId, Pending)> = Vec::new();
        for (blob, pending) in &store.pending {
            let found = stored.query_row(params![&blob[..]], |_| Ok(()));
            if found.optional()?.is_none() {
                fresh.push((*blob, *pending));
            }
        }
        fresh.sort_unstable_by(|a, b| {
            let key = |(blob, p): &(BlobId, Pending)| (p.entry, p.digest, *blob);
            key(a).cmp(&key(b))
        });
        let mut i = 0;
        while i < fresh.len() {
            let entry = fresh[i].1.entry;
            let end = fresh[i..]
                .iter()
                .position(|f| f.1.entry != entry)
                .map_or(fresh.len(), |n| i + n);
            self.entry(tx, store, entry, &fresh[i..end])?;
            i = end;
        }
        Ok(objectless)
    }

    /// Writes the new texts of one entry: each to a row the entry holds with the same bytes, or
    /// to a new row against the entry's keyframe, or as a keyframe once its block is full.
    fn entry(
        &mut self,
        tx: &Transaction<'_>,
        store: &Store,
        entry: BinHash,
        fresh: &[(BlobId, Pending)],
    ) -> Result<(), CensusError> {
        let mut held = Self::held(tx, entry)?;
        let mut insert = tx.prepare_cached(
            "INSERT INTO object_bytes (id, keyframe, dict, frame) VALUES (?1, ?2, ?3, ?4)",
        )?;
        let mut text =
            tx.prepare_cached("INSERT INTO text_bytes (blob, class, object) VALUES (?1, ?2, ?3)")?;
        for (blob, pending) in fresh {
            let bytes = store.bytes(pending);
            let mut same = None;
            for row in &held {
                if stated_size(&row.frame).ok() == Some(bytes.len())
                    && self.decode(&held, row)? == bytes
                {
                    same = Some(row.id);
                    break;
                }
            }
            let id = match same {
                Some(id) => id,
                None => {
                    let current = held.last().map(|row| row.keyframe);
                    let block =
                        current.map_or(0, |k| held.iter().filter(|r| r.keyframe == k).count());
                    let id = self.next;
                    self.next += 1;
                    let (keyframe, dict, frame) = match current.filter(|_| block < self.every) {
                        Some(k) => {
                            let key = held.iter().find(|r| r.id == k);
                            let key = key.ok_or(CensusError::ObjectKeyframe {
                                object: id,
                                keyframe: k,
                            })?;
                            let base = self.decode(&held, key)?;
                            let with = With::Raw(&base);
                            let frame = compress(
                                &mut self.cctx,
                                &mut self.scratch,
                                self.level,
                                &bytes,
                                with,
                            );
                            (k, None, frame)
                        }
                        None => {
                            let d = dict_of(&self.dicts, pending.class);
                            let with = d.map_or(With::Nothing, |d| With::Dict(&self.dicts[d].1));
                            let frame = compress(
                                &mut self.cctx,
                                &mut self.scratch,
                                self.level,
                                &bytes,
                                with,
                            );
                            (id, d.map(|d| self.dicts[d].0), frame)
                        }
                    };
                    insert.execute(params![id, keyframe, dict, frame])?;
                    held.push(Held {
                        id,
                        keyframe,
                        dict,
                        frame,
                    });
                    id
                }
            };
            text.execute(params![&blob[..], pending.class.0, id])?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_object_past_the_reads_bound_is_objectless() {
        let object = |len: usize| TextObject {
            class: BinHash(2),
            bytes: vec![0; len],
        };
        let at = Position(0);
        let within = Fresh::of(BinHash(1), at, vec![object(MAX_OBJECT)]);
        assert!(matches!(within, Fresh::One(_)));
        let past = Fresh::of(BinHash(1), at, vec![object(MAX_OBJECT + 1)]);
        assert!(matches!(past, Fresh::Objectless));
    }
}
