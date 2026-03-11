use crate::core::{SmellDetector, SmellType, TestFile, TestSmell};

pub struct ConditionalLogicDetector;

impl SmellDetector for ConditionalLogicDetector {
    fn name(&self) -> &'static str {
        "ConditionalTestLogic"
    }

    fn detect(&self, test_file: &TestFile) -> Vec<TestSmell> {
        test_file
            .test_functions
            .iter()
            .filter(|f| f.has_conditional)
            .map(|f| {
                TestSmell::new(
                    SmellType::ConditionalTestLogic,
                    &test_file.path,
                    f.line,
                    Some(f.name.clone()),
                )
            })
            .collect()
    }
}
