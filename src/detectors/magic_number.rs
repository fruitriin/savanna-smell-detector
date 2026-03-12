use crate::core::{SmellDetector, SmellType, TestFile, TestSmell};

/// デフォルトのホワイトリスト（一般的な定数）
pub const DEFAULT_WHITELIST: &[i64] = &[0, 1, -1, 2];

pub struct MagicNumberTestDetector {
    /// ホワイトリスト（このリストに含まれる数値は無視する）
    pub whitelist: Vec<i64>,
}

impl Default for MagicNumberTestDetector {
    fn default() -> Self {
        Self {
            whitelist: DEFAULT_WHITELIST.to_vec(),
        }
    }
}

impl MagicNumberTestDetector {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_whitelist(mut self, extra: Vec<i64>) -> Self {
        self.whitelist.extend(extra);
        self
    }
}

impl SmellDetector for MagicNumberTestDetector {
    fn name(&self) -> &'static str {
        "MagicNumberTest"
    }

    fn detect(&self, test_file: &TestFile) -> Vec<TestSmell> {
        test_file
            .test_functions
            .iter()
            .filter(|f| {
                f.magic_numbers
                    .iter()
                    .any(|(n, _)| !self.whitelist.contains(n))
            })
            .map(|f| {
                TestSmell::new(
                    SmellType::MagicNumberTest,
                    &test_file.path,
                    f.line,
                    Some(f.name.clone()),
                )
            })
            .collect()
    }
}
