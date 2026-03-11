use crate::core::{SmellDetector, SmellType, TestFile, TestSmell};

pub struct IgnoredTestDetector;

impl SmellDetector for IgnoredTestDetector {
    fn name(&self) -> &'static str {
        "IgnoredTest"
    }

    fn detect(&self, test_file: &TestFile) -> Vec<TestSmell> {
        test_file
            .test_functions
            .iter()
            .filter(|f| f.is_ignored)
            .map(|f| {
                TestSmell::new(
                    SmellType::IgnoredTest,
                    &test_file.path,
                    f.line,
                    Some(f.name.clone()),
                )
            })
            .collect()
    }
}
