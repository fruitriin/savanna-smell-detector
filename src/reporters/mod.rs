mod console;
mod json;

pub use console::ConsoleReporter;
pub use json::JsonReporter;

use crate::core::TestSmell;

/// レポート出力トレイト
pub trait SmellReporter {
    fn report(&self, smells: &[TestSmell]) -> String;
}
