//! Text of a mod made safe to print. A mod's name, its description and its entry names are the
//! mod author's, and a terminal acts on some characters.

use std::borrow::Cow;
use std::fmt;
use std::sync::Arc;

/// Whether a character may not reach a terminal as it is: C0 controls but newline and tab
/// (escape sequences, carriage return), DEL, C1 controls (an 8-bit CSI), and the bidirectional
/// controls that reorder what follows them.
pub fn is_unsafe(c: char) -> bool {
    matches!(
        c,
        '\u{0}'..='\u{8}'
            | '\u{b}'..='\u{1f}'
            | '\u{7f}'..='\u{9f}'
            | '\u{61c}'
            | '\u{200e}'
            | '\u{200f}'
            | '\u{202a}'..='\u{202e}'
            | '\u{2066}'..='\u{2069}'
    )
}

fn escape(text: &str, spell: impl Fn(char) -> String) -> Cow<'_, str> {
    if !text.chars().any(is_unsafe) {
        return Cow::Borrowed(text);
    }
    Cow::Owned(
        text.chars()
            .map(|c| {
                if is_unsafe(c) {
                    spell(c)
                } else {
                    c.to_string()
                }
            })
            .collect(),
    )
}

/// Text for a terminal: every unsafe character written as `\u{..}`. Reforge's own output holds
/// none, so nothing it means is lost.
pub fn clean(text: &str) -> Cow<'_, str> {
    escape(text, |c| format!("\\u{{{:x}}}", c as u32))
}

/// Serialised JSON for a terminal or a file: every unsafe character, which can only stand inside
/// a string, written as a JSON escape, so a parser reads back the same text.
pub fn clean_json(json: &str) -> Cow<'_, str> {
    escape(json, |c| format!("\\u{:04x}", c as u32))
}

/// Text a mod spells: a name, an author, an entry name, a path. It is kept as written and
/// prints cleaned, so it cannot reach a terminal any other way.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Spelled(Arc<str>);

impl Spelled {
    /// Wraps text read from a mod.
    pub fn new(text: impl Into<Arc<str>>) -> Self {
        Self(text.into())
    }

    /// The text as the mod wrote it. Not for a terminal and not for a path on disk.
    pub fn raw(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Spelled {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&clean(&self.0))
    }
}

impl fmt::Debug for Spelled {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", clean(&self.0))
    }
}

impl serde::Serialize for Spelled {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn controls_are_escaped_and_plain_text_is_borrowed() {
        assert!(matches!(
            clean("Lux, the Lady of Luminosity\n\tv1"),
            Cow::Borrowed(_)
        ));
        assert_eq!(clean("a\u{1b}[31mb"), "a\\u{1b}[31mb");
        assert_eq!(clean("\u{202e}txt.exe"), "\\u{202e}txt.exe");
        assert_eq!(clean_json("{\"n\":\"a\u{1b}b\"}"), "{\"n\":\"a\\u001bb\"}");
    }

    #[test]
    fn a_spelled_name_prints_cleaned() {
        let name = Spelled::new("mod\u{7}name");
        assert_eq!(name.to_string(), "mod\\u{7}name");
        assert_eq!(name.raw(), "mod\u{7}name");
    }
}
