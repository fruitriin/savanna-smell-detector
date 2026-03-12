use crate::core::{SmellDetector, SmellType, TestFile, TestSmell};

pub struct AssertionRouletteDetector;

impl SmellDetector for AssertionRouletteDetector {
    fn name(&self) -> &'static str {
        "AssertionRoulette"
    }

    fn detect(&self, test_file: &TestFile) -> Vec<TestSmell> {
        test_file
            .test_functions
            .iter()
            .filter(|f| f.assertion_count >= 2)
            .map(|f| {
                // assert! が1つ以上あれば Strict（メッセージがないと何が失敗したか不明）
                // assert_eq!/assert_ne! のみなら通常（値は自動表示される）
                let smell_type = if f.assert_only_count >= 1 {
                    SmellType::AssertionRouletteStrict
                } else {
                    SmellType::AssertionRoulette
                };
                TestSmell::new(
                    smell_type,
                    &test_file.path,
                    f.line,
                    Some(f.name.clone()),
                )
            })
            .collect()
    }
}
