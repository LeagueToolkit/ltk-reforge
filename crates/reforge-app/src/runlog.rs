//! The details of a run as the window shows them: the library's `tracing` spans, one per
//! stage with its fields and its time, its debug events, and what other crates warn of. A
//! [`Capture`] takes them for the run its thread drives; a line from a thread no run drives
//! goes to the capture started last. Outside a capture, a line is dropped. Every line passes
//! through `clean`, since a span's fields hold a mod's file name.

use std::cell::RefCell;
use std::fmt::Write as _;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use parking_lot::Mutex;
use reforge::model::clean;
use serde::{Deserialize, Serialize};
use tracing::field::{Field, Visit};
use tracing::span::{Attributes, Id};
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::Layer;
use tracing_subscriber::layer::Context;
use tracing_subscriber::registry::LookupSpan;

/// Lines past this many are counted and not kept: a crate that warns per chunk would fill
/// the window.
pub const MAX_LINES: usize = 2000;

/// One line of a run's details.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct LogLine {
    /// Milliseconds since the run started.
    pub at: u64,
    /// `error`, `warn`, `info`, `debug` or `trace`.
    pub level: String,
    /// The line, cleaned.
    pub text: String,
}

struct Active {
    started: Instant,
    sink: Box<dyn Fn(LogLine) + Send + Sync>,
    kept: AtomicUsize,
}

thread_local! {
    /// The capture of the run this thread drives: its stages' spans open on it.
    static HERE: RefCell<Option<Arc<Active>>> = const { RefCell::new(None) };
}

/// The capture started last, for a line from a thread no run drives, as a worker pool's.
static LATEST: Mutex<Option<Arc<Active>>> = Mutex::new(None);

/// The lines of one run, taken on the thread that drives it while the capture lives.
pub struct Capture(Arc<Active>);

impl Capture {
    /// Sends each line to `sink` until the capture is dropped.
    pub fn start(sink: impl Fn(LogLine) + Send + Sync + 'static) -> Self {
        let active = Arc::new(Active {
            started: Instant::now(),
            sink: Box::new(sink),
            kept: AtomicUsize::new(0),
        });
        HERE.with(|here| *here.borrow_mut() = Some(Arc::clone(&active)));
        *LATEST.lock() = Some(Arc::clone(&active));
        Self(active)
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        HERE.with(|here| here.borrow_mut().take());
        let mut latest = LATEST.lock();
        if latest
            .as_ref()
            .is_some_and(|latest| Arc::ptr_eq(latest, &self.0))
        {
            *latest = None;
        }
    }
}

fn send(level: Level, text: String) {
    let here = HERE.with(|here| here.borrow().clone());
    let Some(active) = here.or_else(|| LATEST.lock().clone()) else {
        return;
    };
    let kept = active.kept.fetch_add(1, Ordering::Relaxed) + 1;
    if kept > MAX_LINES {
        return;
    }
    let at = u64::try_from(active.started.elapsed().as_millis()).unwrap_or(u64::MAX);
    let text = match kept == MAX_LINES {
        true => format!("{MAX_LINES} lines; the rest are not kept"),
        false => clean(&text).into_owned(),
    };
    (active.sink)(LogLine {
        at,
        level: level.as_str().to_ascii_lowercase(),
        text,
    });
}

/// The fields of a span or an event, as `name=value` after its message.
#[derive(Default)]
struct Fields {
    message: String,
    rest: String,
}

impl Visit for Fields {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            let _ = write!(self.message, "{value:?}");
        } else {
            let _ = write!(self.rest, " {}={value:?}", field.name());
        }
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "message" {
            self.message.push_str(value);
        } else {
            let _ = write!(self.rest, " {}={value}", field.name());
        }
    }
}

/// A span's start and what it said of itself.
struct Opened {
    at: Instant,
    text: String,
}

/// The layer that feeds a [`Capture`].
pub struct RunLogLayer;

impl<S: Subscriber + for<'a> LookupSpan<'a>> Layer<S> for RunLogLayer {
    fn on_new_span(&self, attrs: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
        let mut fields = Fields::default();
        attrs.record(&mut fields);
        let text = format!("{}{}", attrs.metadata().name(), fields.rest);
        if let Some(span) = ctx.span(id) {
            span.extensions_mut().insert(Opened {
                at: Instant::now(),
                text,
            });
        }
    }

    fn on_close(&self, id: Id, ctx: Context<'_, S>) {
        let Some(span) = ctx.span(&id) else {
            return;
        };
        let level = *span.metadata().level();
        if let Some(opened) = span.extensions().get::<Opened>() {
            let millis = opened.at.elapsed().as_millis();
            send(level, format!("{} ({millis} ms)", opened.text));
        }
    }

    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let mut fields = Fields::default();
        event.record(&mut fields);
        let meta = event.metadata();
        send(
            *meta.level(),
            format!("{}: {}{}", meta.target(), fields.message, fields.rest),
        );
    }
}
