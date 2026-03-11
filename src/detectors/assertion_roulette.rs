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
                TestSmell::new(
                    SmellType::AssertionRoulette,
                    &test_file.path,
                    f.line,
                    Some(f.name.clone()),
                )
            })
            .collect()
    }
}
