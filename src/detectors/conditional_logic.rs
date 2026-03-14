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
                // 真の条件分岐（if/match）がある場合はスメル
                if f.has_branching {
                    return true;
                }
                // for ループのみ（if/match なし）→ テーブル駆動 or 状態構築、いずれもスメルではない
                if f.has_for_loop {
                    return false;
                }
                // while/loop はスメル（has_conditional が true で、has_branching=false, has_for_loop=false）
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
