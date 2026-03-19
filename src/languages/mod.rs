mod rust;
mod shell;

pub use rust::RustParser;
pub use shell::ShellParser;

use crate::core::TestFile;

/// 言語パーサーのトレイト
pub trait LanguageParser: Send + Sync {
    /// 対応するファイル拡張子
    fn extensions(&self) -> &[&str];

    /// 言語名
    fn language_name(&self) -> &str;

    /// ソースコードを解析して TestFile を返す
    fn parse(&self, path: &str, source: &str) -> Option<TestFile>;
}
