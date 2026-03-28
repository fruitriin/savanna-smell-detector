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
            .filter(|f| {
                if f.is_empty || f.has_assertion {
                    return false;
                }
                // Go の Benchmark*/Example* はアサーション不要
                // (Benchmark はパフォーマンス測定、Example は // Output: コメントが実質アサーション)
                if f.name.starts_with("Benchmark") || f.name.starts_with("Example") {
                    return false;
                }
                true
            })
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
