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
            // sleep と時間API の両方を使っている場合のみ検出
            // Instant::now() 単体での時刻算術（sleep なし）は安定しているため除外
            .filter(|f| f.has_timeout_dependency && f.has_sleep)
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
