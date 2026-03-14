use crate::core::{SmellDetector, SmellType, TestFile, TestSmell};

pub struct AssertionRouletteDetector {
    pub threshold: usize,
}

impl Default for AssertionRouletteDetector {
    fn default() -> Self {
        Self { threshold: 2 }
    }
}

impl AssertionRouletteDetector {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_threshold(mut self, threshold: usize) -> Self {
        self.threshold = threshold;
        self
    }
}

impl SmellDetector for AssertionRouletteDetector {
    fn name(&self) -> &'static str {
        "AssertionRoulette"
    }

    fn detect(&self, test_file: &TestFile) -> Vec<TestSmell> {
        let mut smells = Vec::new();

        for f in &test_file.test_functions {
            let total_without_msg =
                f.assertions_without_message + f.assert_only_without_message;

            // Strict: メッセージなし assert!/debug_assert! が1つ以上 かつ
            //         メッセージなしアサーション合計が閾値以上
            if f.assert_only_without_message >= 1 && total_without_msg >= self.threshold {
                smells.push(TestSmell::new(
                    SmellType::AssertionRouletteStrict,
                    &test_file.path,
                    f.line,
                    Some(f.name.clone()),
                ));
            // 通常: メッセージなし assert_eq!/assert_ne! が閾値以上
            } else if f.assertions_without_message >= self.threshold {
                smells.push(TestSmell::new(
                    SmellType::AssertionRoulette,
                    &test_file.path,
                    f.line,
                    Some(f.name.clone()),
                ));
            }
        }

        smells
    }
}
