use crate::core::{SmellDetector, SmellType, TestFile, TestSmell};

pub struct NoTestDetector;

impl SmellDetector for NoTestDetector {
    fn name(&self) -> &'static str {
        "NoTest"
    }

    fn detect(&self, test_file: &TestFile) -> Vec<TestSmell> {
        if test_file.test_functions.is_empty() {
            vec![TestSmell::new(
                SmellType::NoTest,
                &test_file.path,
                1,
                None,
            )]
        } else {
            vec![]
        }
    }
}
