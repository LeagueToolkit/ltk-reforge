//! What the command prints. Text may carry a mod's own names, so every line goes through
//! `clean`, which escapes each character a terminal would act on.

/// A line on standard output, cleaned.
macro_rules! outln {
    ($($t:tt)*) => { println!("{}", reforge::model::clean(&format!($($t)*))) };
}

/// Text on standard output with no line end added, cleaned.
macro_rules! out {
    ($($t:tt)*) => { print!("{}", reforge::model::clean(&format!($($t)*))) };
}

/// A line on standard error, cleaned.
macro_rules! errln {
    ($($t:tt)*) => { eprintln!("{}", reforge::model::clean(&format!($($t)*))) };
}
