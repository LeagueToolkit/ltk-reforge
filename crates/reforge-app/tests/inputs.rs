//! The inputs: opened once, and what each one found raised into every run's log as the command
//! raises it. Every test names its hash cache and schema, so none reads LTK Manager's copies on
//! the machine.

use camino::{Utf8Path, Utf8PathBuf};
use fs_err as fs;
use reforge_app::error::AppError;
use reforge_app::inputs::{Choice, Inputs};
use tempfile::TempDir;

const VERSION: &str = "16.19.8207193+branch.releases-16-19.content.release";

/// A folder for a test, with an install of no WADs at a build in it.
struct Machine {
    _dir: TempDir,
    root: Utf8PathBuf,
}

impl Machine {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
        let game = root.join("League of Legends");
        fs::create_dir_all(game.join("Game/DATA/FINAL/Champions")).unwrap();
        fs::write(
            game.join("Game/content-metadata.json"),
            format!(r#"{{ "version": "{VERSION}" }}"#),
        )
        .unwrap();
        fs::create_dir_all(root.join("hashes")).unwrap();
        Self { _dir: dir, root }
    }

    fn path(&self, under: &str) -> Utf8PathBuf {
        self.root.join(under)
    }

    /// The install, an empty hash cache and a schema file that is not there.
    fn choice(&self) -> Choice {
        Choice {
            game: Some(self.path("League of Legends")),
            hashes: Some(self.path("hashes")),
            schema: Some(self.path("meta-schema.json")),
            history: None,
        }
    }

    fn open(&self, choice: &Choice) -> Result<Inputs, AppError> {
        Inputs::open(choice, &self.path("names-index"))
    }
}

fn codes(inputs: &Inputs) -> Vec<String> {
    inputs
        .view()
        .findings
        .into_iter()
        .map(|finding| finding.code)
        .collect()
}

#[test]
fn no_game_chosen_opens_nothing() {
    let machine = Machine::new();
    let choice = Choice {
        game: None,
        ..machine.choice()
    };

    assert!(matches!(machine.open(&choice), Err(AppError::NoGame)));
}

#[test]
fn a_folder_with_no_game_in_it_names_the_folder() {
    let machine = Machine::new();
    let choice = Choice {
        game: Some(machine.path("hashes")),
        ..machine.choice()
    };

    let Err(AppError::GameUnreadable { path, .. }) = machine.open(&choice) else {
        panic!("a folder with no game in it opened");
    };
    assert_eq!(Utf8Path::new(&path), machine.path("hashes"));
}

#[test]
fn each_input_that_is_absent_is_a_finding_in_the_commands_order() {
    let machine = Machine::new();

    let inputs = machine.open(&machine.choice()).unwrap();

    assert_eq!(
        codes(&inputs),
        ["names.mimir-absent", "history.absent", "schema.unreadable"]
    );
}

#[test]
fn a_history_index_that_does_not_open_is_an_error_naming_its_folder() {
    let machine = Machine::new();
    let choice = Choice {
        history: Some(machine.path("no-index")),
        ..machine.choice()
    };

    let inputs = machine.open(&choice).unwrap();

    let view = inputs.view();
    let missing = view
        .findings
        .iter()
        .find(|finding| finding.code == "history.index-missing")
        .unwrap_or_else(|| panic!("no history.index-missing in {:?}", codes(&inputs)));
    assert!(missing.message.contains("no-index"), "{}", missing.message);
    assert_eq!(view.history.dir, None);
}

#[test]
fn every_run_starts_from_the_same_findings() {
    let machine = Machine::new();
    let inputs = machine.open(&machine.choice()).unwrap();

    let first: Vec<_> = inputs.log().iter().map(|d| d.code.name).collect();
    let second: Vec<_> = inputs.log().iter().map(|d| d.code.name).collect();

    assert_eq!(first, second);
    assert_eq!(first.len(), 3);
}

#[test]
fn the_view_names_the_install_and_its_build() {
    let machine = Machine::new();

    let view = machine.open(&machine.choice()).unwrap().view();

    assert_eq!(
        Utf8Path::new(&view.game.path),
        machine.path("League of Legends")
    );
    assert_eq!(view.game.build.as_deref(), Some("16.19.8207193"));
    assert_eq!(view.schema.path, None);
}
