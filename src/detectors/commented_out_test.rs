use crate::core::{SmellDetector, SmellType, TestFile, TestSmell};

pub struct CommentedOutTestDetector;

impl SmellDetector for CommentedOutTestDetector {
    fn name(&self) -> &'static str {
        "CommentedOutTest"
    }

    fn detect(&self, test_file: &TestFile) -> Vec<TestSmell> {
        let source = match &test_file.source {
            Some(s) => s,
            None => return Vec::new(),
        };

        let mut results = Vec::new();
        for (i, line) in source.lines().enumerate() {
            let trimmed = line.trim();
            // // #[test] パターン
            if trimmed.starts_with("//") && trimmed.contains("#[test]") {
                results.push(TestSmell::new(
                    SmellType::CommentedOutTest,
                    &test_file.path,
                    i + 1,
                    None,
                ));
            }
            // /* ... #[test] ... */ パターン
            if trimmed.starts_with("/*") && trimmed.contains("#[test]") {
                results.push(TestSmell::new(
                    SmellType::CommentedOutTest,
                    &test_file.path,
                    i + 1,
                    None,
                ));
            }
        }
        results
    }
}
