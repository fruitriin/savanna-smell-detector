use super::{TestFile, TestSmell};

/// テスト臭いを検出するトレイト
pub trait SmellDetector: Send + Sync {
    /// 検出器の名前
    fn name(&self) -> &'static str;

    /// ファイル単位で臭いを検出
    fn detect(&self, test_file: &TestFile) -> Vec<TestSmell>;
}
