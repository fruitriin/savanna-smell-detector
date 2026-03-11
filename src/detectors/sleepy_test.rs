use crate::core::{SmellDetector, SmellType, TestFile, TestSmell};

pub struct SleepyTestDetector;

impl SmellDetector for SleepyTestDetector {
    fn name(&self) -> &'static str {
        "SleepyTest"
    }

    fn detect(&self, test_file: &TestFile) -> Vec<TestSmell> {
        test_file
            .test_functions
            .iter()
            .filter(|f| f.has_sleep)
            .map(|f| {
                TestSmell::new(
                    SmellType::SleepyTest,
                    &test_file.path,
                    f.line,
                    Some(f.name.clone()),
                )
            })
            .collect()
    }
}
