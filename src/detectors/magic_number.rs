use crate::core::{SmellDetector, SmellType, TestFile, TestSmell};

pub struct MagicNumberTestDetector;

impl SmellDetector for MagicNumberTestDetector {
    fn name(&self) -> &'static str {
        "MagicNumberTest"
    }

    fn detect(&self, test_file: &TestFile) -> Vec<TestSmell> {
        test_file
            .test_functions
            .iter()
            .filter(|f| !f.magic_numbers.is_empty())
            .map(|f| {
                TestSmell::new(
                    SmellType::MagicNumberTest,
                    &test_file.path,
                    f.line,
                    Some(f.name.clone()),
                )
            })
            .collect()
    }
}
