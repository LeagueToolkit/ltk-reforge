//! The questions the History page asks: each the library's answer over a history in memory,
//! its hashes spelled through text tables made for the test.

use reforge::history::NoHistory;
use reforge_app::error::AppError;
use reforge_app::lookup::{BuildView, Lookup};
use reforge_testkit::history::MemoryHistory;

fn history() -> MemoryHistory {
    MemoryHistory::new(&["14.1.1", "14.2.1", "14.1.2", "14.3.1"])
}

#[test]
fn no_history_is_refused_as_no_index() {
    assert!(matches!(Lookup::new(&NoHistory), Err(AppError::NoIndex)));
}

#[test]
fn the_builds_come_in_position_order_with_the_head() {
    let history = history();
    let builds = Lookup::new(&history).unwrap().builds();

    assert_eq!(builds.head, 3);
    let versions: Vec<&str> = builds.builds.iter().map(|b| b.version.as_str()).collect();
    assert_eq!(versions, ["14.1.1", "14.2.1", "14.1.2", "14.3.1"]);
    assert_eq!(
        builds.builds[2],
        BuildView {
            position: 2,
            version: "14.1.2".into(),
            commit: "c2".into(),
        }
    );
}
