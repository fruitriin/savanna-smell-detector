mod console;
mod json;
mod markdown;

pub use console::ConsoleReporter;
pub use json::JsonReporter;
pub use markdown::MarkdownReporter;

use crate::core::TestSmell;

/// レポート出力トレイト
pub trait SmellReporter {
    fn report(&self, smells: &[TestSmell]) -> String;
}
