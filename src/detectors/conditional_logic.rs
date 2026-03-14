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
            .filter(|f| {
                // 真の条件分岐（if/match）は常にスメル
                if f.has_branching {
                    return true;
                }
                // for ループ内にアサーションがある場合はテーブル駆動テストの可能性が高い → スメルではない
                if f.has_for_loop && f.has_assertion_in_loop {
                    return false;
                }
                // それ以外のループ（while/loop、アサーションなしの for）はスメル
                f.has_conditional
            })
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
