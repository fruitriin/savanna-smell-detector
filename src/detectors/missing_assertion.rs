use crate::core::{SmellDetector, SmellType, TestFile, TestSmell};

pub struct MissingAssertionDetector;

impl SmellDetector for MissingAssertionDetector {
    fn name(&self) -> &'static str {
        "MissingAssertion"
    }

    fn detect(&self, test_file: &TestFile) -> Vec<TestSmell> {
        test_file
            .test_functions
            .iter()
            .filter(|f| !f.is_empty && !f.has_assertion)
            .map(|f| {
                TestSmell::new(
                    SmellType::MissingAssertion,
                    &test_file.path,
                    f.line,
                    Some(f.name.clone()),
                )
            })
            .collect()
    }
}
