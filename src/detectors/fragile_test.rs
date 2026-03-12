use crate::core::{SmellDetector, SmellType, TestFile, TestSmell};

pub struct FragileTestDetector;

impl SmellDetector for FragileTestDetector {
    fn name(&self) -> &'static str {
        "FragileTest"
    }

    fn detect(&self, test_file: &TestFile) -> Vec<TestSmell> {
        test_file
            .test_functions
            .iter()
            .filter(|f| f.has_timeout_dependency)
            .map(|f| {
                TestSmell::new(
                    SmellType::FragileTest,
                    &test_file.path,
                    f.line,
                    Some(f.name.clone()),
                )
            })
            .collect()
    }
}
