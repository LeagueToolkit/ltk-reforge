//! A run's details: the library's spans and events, taken by the run that raised them. A file
//! of its own, so its process has one subscriber for every thread, as the app has: `tracing`
//! caches per callsite whether any subscriber wants it, across a process's threads.

use std::sync::{Arc, Mutex};

use camino::Utf8PathBuf;
use fs_err as fs;
use reforge::history::NoHistory;
use reforge_app::inputs::Inputs;
use reforge_app::run::{self, RunKind, RunRequest, RunSettings, StageName, Watch};
use reforge_app::runlog::{LogLine, RunLogLayer};
use reforge_testkit::bins::bin;
use reforge_testkit::facts::{archive, env, own_files, own_graph, own_skin};
use reforge_testkit::skins::{self, skin_bin};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

#[derive(Default)]
struct Lines(Arc<Mutex<Vec<LogLine>>>);

impl Watch for Lines {
    fn log(&mut self) -> Option<Box<dyn Fn(LogLine) + Send + Sync>> {
        let lines = Arc::clone(&self.0);
        Some(Box::new(move |line| lines.lock().unwrap().push(line)))
    }
}

#[test]
fn the_details_hold_the_librarys_span_of_each_stage_with_its_time() {
    tracing_subscriber::registry().with(RunLogLayer).init();
    let dir = tempfile::tempdir().unwrap();
    let path = Utf8PathBuf::from_path_buf(dir.path().join("mod.fantome")).unwrap();
    let objects = vec![
        reforge_testkit::bins::entry(
            "Characters/Test/Skins/Skin0",
            &own_skin(
                "a",
                "Characters/Test/Animations/Skin0",
                "Characters/Test/Skins/Skin0/Resources",
            ),
        ),
        own_graph("a", "Characters/Test/Animations/Skin0"),
    ];
    let mut files = own_files("a");
    files.push((skin_bin("test", 0), bin(objects)));
    fs::write(&path, archive(&files).build()).unwrap();
    let inputs = Inputs::of_env(env(skins::game(), NoHistory));
    let request = RunRequest {
        kind: RunKind::Repair,
        archive: path.to_string(),
        through: StageName::Fix,
        settings: RunSettings::default(),
        out: None,
    };
    let mut heard = Lines::default();

    let view = run::run(&inputs, &request, &mut heard).unwrap();

    let texts: Vec<&str> = view.log.iter().map(|line| line.text.as_str()).collect();
    for stage in ["read", "fix"] {
        assert!(
            texts
                .iter()
                .any(|text| text.starts_with(stage) && text.ends_with(" ms)")),
            "no span of {stage} in {texts:?}"
        );
    }
    assert_eq!(view.log, *heard.0.lock().unwrap());
}
