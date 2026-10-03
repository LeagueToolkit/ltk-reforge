//! `META/info.json`: what the archive says of itself.

use crate::model::Spelled;

/// Why `META/info.json` gives no metadata.
#[derive(Debug, thiserror::Error)]
pub(super) enum InfoError {
    /// The text is not JSON.
    #[error("not JSON: {0}")]
    Json(#[from] serde_json::Error),
    /// The JSON is not an object.
    #[error("not a JSON object")]
    NotAnObject,
}

/// The fields of `META/info.json`. A field that is absent, empty or no string is none.
#[derive(Debug, Default)]
pub(super) struct Info {
    /// `Name`.
    pub(super) name: Option<Spelled>,
    /// `Author`.
    pub(super) author: Option<Spelled>,
    /// `Version`.
    pub(super) version: Option<Spelled>,
    /// `Description`.
    pub(super) description: Option<Spelled>,
}

impl Info {
    /// Reads the entry's bytes. Packers write a byte-order mark and spell the keys in any case.
    pub(super) fn parse(bytes: &[u8]) -> Result<Self, InfoError> {
        let text = String::from_utf8_lossy(bytes);
        let value: serde_json::Value = serde_json::from_str(text.trim_start_matches('\u{feff}'))?;
        let object = value.as_object().ok_or(InfoError::NotAnObject)?;
        let field = |key: &str| {
            object
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(key))
                .and_then(|(_, v)| v.as_str())
                .map(str::trim)
                .filter(|text| !text.is_empty())
                .map(Spelled::new)
        };
        Ok(Self {
            name: field("Name"),
            author: field("Author"),
            version: field("Version"),
            description: field("Description"),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_field_is_trimmed_and_an_empty_or_numeric_one_is_none() {
        let info =
            Info::parse(br#"{"name": "  Lux  ", "AUTHOR": "", "Version": 2, "x": 1}"#).unwrap();
        assert_eq!(info.name, Some(Spelled::new("Lux")));
        assert_eq!(info.author, None);
        assert_eq!(info.version, None);
        assert_eq!(info.description, None);
    }

    #[test]
    fn text_that_is_no_json_object_is_refused() {
        assert!(matches!(Info::parse(b"{"), Err(InfoError::Json(_))));
        assert!(matches!(Info::parse(b"[]"), Err(InfoError::NotAnObject)));
    }
}
