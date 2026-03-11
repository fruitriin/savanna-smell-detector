use crate::core::{SmellDetector, SmellType, TestFile, TestSmell};

pub struct RedundantPrintDetector;

impl SmellDetector for RedundantPrintDetector {
    fn name(&self) -> &'static str {
        "RedundantPrint"
    }

    fn detect(&self, test_file: &TestFile) -> Vec<TestSmell> {
        test_file
            .test_functions
            .iter()
            .filter(|f| f.has_print)
            .map(|f| {
                TestSmell::new(
                    SmellType::RedundantPrint,
                    &test_file.path,
                    f.line,
                    Some(f.name.clone()),
                )
            })
            .collect()
    }
}
