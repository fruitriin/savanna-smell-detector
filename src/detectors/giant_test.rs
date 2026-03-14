use crate::core::{SmellDetector, SmellType, TestFile, TestSmell};

pub const DEFAULT_GIANT_TEST_THRESHOLD: usize = 50;

pub struct GiantTestDetector {
    pub threshold: usize,
}

impl Default for GiantTestDetector {
    fn default() -> Self {
        Self { threshold: DEFAULT_GIANT_TEST_THRESHOLD }
    }
}

impl GiantTestDetector {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_threshold(mut self, threshold: usize) -> Self {
        self.threshold = threshold;
        self
    }
}

impl SmellDetector for GiantTestDetector {
    fn name(&self) -> &'static str {
        "GiantTest"
    }

    fn detect(&self, test_file: &TestFile) -> Vec<TestSmell> {
        test_file
            .test_functions
            .iter()
            .filter(|f| f.body_line_count >= self.threshold)
            .map(|f| {
                TestSmell::new(
                    SmellType::GiantTest,
                    &test_file.path,
                    f.line,
                    Some(f.name.clone()),
                )
            })
            .collect()
    }
}
