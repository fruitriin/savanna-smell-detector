use crate::core::{SmellDetector, SmellType, TestFile, TestSmell};

pub struct SilentSkipDetector;

impl SmellDetector for SilentSkipDetector {
    fn name(&self) -> &'static str {
        "SilentSkip"
    }

    fn detect(&self, test_file: &TestFile) -> Vec<TestSmell> {
        test_file
            .test_functions
            .iter()
            .filter(|f| f.has_early_return)
            .map(|f| {
                TestSmell::new(
                    SmellType::SilentSkip,
                    &test_file.path,
                    f.line,
                    Some(f.name.clone()),
                )
            })
            .collect()
    }
}
