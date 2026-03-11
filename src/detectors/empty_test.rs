use crate::core::{SmellDetector, SmellType, TestFile, TestSmell};

pub struct EmptyTestDetector;

impl SmellDetector for EmptyTestDetector {
    fn name(&self) -> &'static str {
        "EmptyTest"
    }

    fn detect(&self, test_file: &TestFile) -> Vec<TestSmell> {
        test_file
            .test_functions
            .iter()
            .filter(|f| f.is_empty)
            .map(|f| {
                TestSmell::new(
                    SmellType::EmptyTest,
                    &test_file.path,
                    f.line,
                    Some(f.name.clone()),
                )
            })
            .collect()
    }
}
