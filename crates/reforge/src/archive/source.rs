//! What an archive is read from: its bytes in memory, or its file, which is read entry by entry
//! and never whole (#ADR-0100). A stored entry is a window of either, read where it lies.

use std::io::{self, BufReader, Cursor, Read, Seek, SeekFrom};

use camino::Utf8Path;
use fs_err as fs;

/// Where an archive's bytes are.
#[derive(Debug, Clone, Copy)]
pub(super) enum Source<'a> {
    /// In memory, borrowed for the read.
    Bytes(&'a [u8]),
    /// In a file of this size.
    File { path: &'a Utf8Path, len: u64 },
}

impl<'a> Source<'a> {
    /// The archive's size.
    pub(super) fn len(&self) -> u64 {
        match self {
            Source::Bytes(bytes) => bytes.len() as u64,
            Source::File { len, .. } => *len,
        }
    }

    /// The archive's bytes, to be read from the start.
    pub(super) fn open(&self) -> io::Result<Input<'a>> {
        Ok(match *self {
            Source::Bytes(bytes) => Input::Bytes(Cursor::new(bytes)),
            Source::File { path, .. } => Input::File(BufReader::new(fs::File::open(path)?)),
        })
    }

    /// `len` bytes from `start`, read in place: a part of the bytes, or the file opened again so
    /// that the zip's reader keeps its own position.
    pub(super) fn window(&self, start: u64, len: u64) -> io::Result<Window<Input<'a>>> {
        Window::new(self.open()?, start, len)
    }
}

/// An archive's bytes, as the zip reader and a window read them.
pub(super) enum Input<'a> {
    Bytes(Cursor<&'a [u8]>),
    File(BufReader<fs::File>),
}

impl Read for Input<'_> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            Input::Bytes(bytes) => bytes.read(buf),
            Input::File(file) => file.read(buf),
        }
    }
}

impl Seek for Input<'_> {
    fn seek(&mut self, to: SeekFrom) -> io::Result<u64> {
        match self {
            Input::Bytes(bytes) => bytes.seek(to),
            Input::File(file) => file.seek(to),
        }
    }
}

/// A part of a reader, read and sought as if it were the whole: `len` bytes from `start`.
#[derive(Debug)]
pub(super) struct Window<R> {
    inner: R,
    start: u64,
    len: u64,
    at: u64,
}

impl<R: Read + Seek> Window<R> {
    /// The part of `inner` from `start`, `len` bytes of it.
    pub(super) fn new(mut inner: R, start: u64, len: u64) -> io::Result<Self> {
        inner.seek(SeekFrom::Start(start))?;
        Ok(Self {
            inner,
            start,
            len,
            at: 0,
        })
    }

    /// Its size.
    pub(super) fn len(&self) -> u64 {
        self.len
    }
}

impl<R: Read> Read for Window<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let left = self.len.saturating_sub(self.at);
        let most = buf.len().min(usize::try_from(left).unwrap_or(usize::MAX));
        let read = self.inner.read(&mut buf[..most])?;
        self.at += read as u64;
        Ok(read)
    }
}

impl<R: Seek> Seek for Window<R> {
    fn seek(&mut self, to: SeekFrom) -> io::Result<u64> {
        let at = match to {
            SeekFrom::Start(at) => Some(at),
            SeekFrom::End(by) => self.len.checked_add_signed(by),
            SeekFrom::Current(by) => self.at.checked_add_signed(by),
        };
        let at = at.ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "a seek before the start of the window",
            )
        })?;
        // A position past the end is allowed, as in a file, and reads nothing.
        self.inner
            .seek(SeekFrom::Start(self.start.saturating_add(at)))?;
        self.at = at;
        Ok(at)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_window_reads_and_seeks_within_its_part_alone() {
        let bytes: Vec<u8> = (0..20).collect();
        let mut window = Window::new(Cursor::new(&bytes[..]), 5, 10).unwrap();
        let mut all = Vec::new();
        window.read_to_end(&mut all).unwrap();
        assert_eq!(all, (5..15).collect::<Vec<u8>>());
        assert_eq!(window.seek(SeekFrom::End(-2)).unwrap(), 8);
        let mut two = [0u8; 4];
        assert_eq!(window.read(&mut two).unwrap(), 2);
        assert_eq!(two[..2], [13, 14]);
        assert_eq!(window.seek(SeekFrom::Start(1)).unwrap(), 1);
        assert_eq!(window.seek(SeekFrom::Current(2)).unwrap(), 3);
        assert_eq!(window.read(&mut two).unwrap(), 4);
        assert_eq!(two, [8, 9, 10, 11]);
        assert!(window.seek(SeekFrom::Current(-8)).is_err());
        assert_eq!(window.seek(SeekFrom::Start(30)).unwrap(), 30);
        assert_eq!(window.read(&mut two).unwrap(), 0);
    }
}
