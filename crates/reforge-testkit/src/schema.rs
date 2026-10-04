//! A meta schema from rows, for tests: classes with their fields, each field with the builds
//! it holds at, written as the database's JSON and read by the reader a run uses.

use reforge::game::Schema;
use reforge::model::name_hash;

/// The first build every class of a test schema exists at.
pub const FIRST: u64 = 100;

fn hex(name: &str) -> String {
    format!("0x{:x}", name_hash(name).0)
}

/// The type of a field, as the database's four slots.
#[derive(Debug, Clone)]
pub struct Type([String; 4]);

/// A field of a kind with no class: `String`, `Bool`, `F32`, `Vec2`, `File`.
pub fn kind(name: &str) -> Type {
    Type([name.to_string(), "0x0".into(), "0x0".into(), "0x0".into()])
}

/// A pointer to a struct of a class.
pub fn pointer(class: &str) -> Type {
    Type(["Pointer".into(), "0x0".into(), "0x0".into(), hex(class)])
}

/// An embedded struct of a class.
pub fn embed(class: &str) -> Type {
    Type(["Embed".into(), "0x0".into(), "0x0".into(), hex(class)])
}

/// A list of items of a kind.
pub fn list(item: &str) -> Type {
    Type(["List".into(), "0x0".into(), item.to_string(), "0x0".into()])
}

/// A list of the second kind, `List2`, of items of a kind.
pub fn list2(item: &str) -> Type {
    Type(["List2".into(), "0x0".into(), item.to_string(), "0x0".into()])
}

/// A list of pointers to structs of a class.
pub fn list_of(class: &str) -> Type {
    Type(["List".into(), "0x0".into(), "Pointer".into(), hex(class)])
}

/// An option of an item of a kind.
pub fn option(item: &str) -> Type {
    Type([
        "Option".into(),
        "0x0".into(),
        item.to_string(),
        "0x0".into(),
    ])
}

/// A map from keys of a kind to values of a kind.
pub fn map(key: &str, value: &str) -> Type {
    Type([
        "Map".into(),
        key.to_string(),
        value.to_string(),
        "0x0".into(),
    ])
}

/// One type of a field over a range of builds.
#[derive(Debug, Clone)]
struct Revision {
    of: Type,
    from: u64,
    to: Option<u64>,
}

/// One field of a class, with each type it had over the builds.
#[derive(Debug, Clone)]
pub struct FieldRow {
    name: String,
    revisions: Vec<Revision>,
}

/// A field a class declares from [`FIRST`] on.
pub fn field(name: &str, of: Type) -> FieldRow {
    let from = FIRST;
    FieldRow {
        name: name.to_string(),
        revisions: vec![Revision { of, from, to: None }],
    }
}

impl FieldRow {
    /// Declared from this build on, where it was declared from [`FIRST`].
    #[must_use]
    pub fn from(mut self, build: u64) -> Self {
        if let Some(last) = self.revisions.last_mut() {
            last.from = build;
        }
        self
    }

    /// Declared up to this build, included.
    #[must_use]
    pub fn until(mut self, build: u64) -> Self {
        if let Some(last) = self.revisions.last_mut() {
            last.to = Some(build);
        }
        self
    }

    /// Of another type from this build on. The type before holds up to the build before it.
    #[must_use]
    pub fn then(mut self, build: u64, of: Type) -> Self {
        self = self.until(build - 1);
        let from = build;
        self.revisions.push(Revision { of, from, to: None });
        self
    }

    fn json(&self) -> String {
        let revision = |revision: &Revision| {
            let to = revision.to;
            let to = to.map_or(String::new(), |to| format!(r#""to": {to}, "#));
            let [kind, key, item, class] = &revision.of.0;
            format!(
                r#"{{ "from": {}, {to}"type": ["{kind}", "{key}", "{item}", "{class}"] }}"#,
                revision.from
            )
        };
        let revisions: Vec<String> = self.revisions.iter().map(revision).collect();
        format!(
            r#""{}": {{ "name": "{}", "revisions": [{}] }}"#,
            hex(&self.name),
            self.name,
            revisions.join(", ")
        )
    }
}

/// A meta schema database built class by class.
#[derive(Debug, Clone)]
pub struct SchemaRows {
    latest: u64,
    classes: Vec<String>,
}

impl SchemaRows {
    /// A database that describes builds from [`FIRST`] to `latest`.
    pub fn new(latest: u64) -> Self {
        Self {
            latest,
            classes: Vec::new(),
        }
    }

    /// Adds a class with no base, which exists from [`FIRST`] on.
    #[must_use]
    pub fn class(self, name: &str, fields: &[FieldRow]) -> Self {
        self.derived(name, &[], fields)
    }

    /// Adds a class with these bases, which exists from [`FIRST`] on.
    #[must_use]
    pub fn derived(mut self, name: &str, bases: &[&str], fields: &[FieldRow]) -> Self {
        let bases: Vec<String> = bases
            .iter()
            .map(|base| format!("\"{}\"", hex(base)))
            .collect();
        let fields: Vec<String> = fields.iter().map(FieldRow::json).collect();
        self.classes.push(format!(
            r#""{}": {{ "name": "{name}", "revisions": [{{ "from": {FIRST}, "bases": [{}] }}], "properties": {{ {} }} }}"#,
            hex(name),
            bases.join(", "),
            fields.join(", ")
        ));
        self
    }

    /// The database's JSON.
    pub fn json(&self) -> String {
        format!(
            r#"{{ "formatVersion": 1, "latest": {}, "classes": {{ {} }} }}"#,
            self.latest,
            self.classes.join(",\n")
        )
    }

    /// The schema, as a run reads it.
    ///
    /// # Panics
    ///
    /// When the rows do not make a database the reader reads.
    pub fn build(&self) -> Schema {
        Schema::parse(self.json().as_bytes())
            .unwrap_or_else(|error| panic!("the test schema does not read: {error}"))
    }
}
