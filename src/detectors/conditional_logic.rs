use crate::core::{SmellDetector, SmellType, TestFile, TestSmell};

pub struct ConditionalLogicDetector;

impl SmellDetector for ConditionalLogicDetector {
    fn name(&self) -> &'static str {
        "ConditionalTestLogic"
    }

    fn detect(&self, test_file: &TestFile) -> Vec<TestSmell> {
        let is_xcuitest = test_file.flavor.as_deref() == Some("xcuitest");
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
                let smell = TestSmell::new(
                    SmellType::ConditionalTestLogic,
                    &test_file.path,
                    f.line,
                    Some(f.name.clone()),
                );
                if !is_xcuitest {
                    return smell;
                }
                // E2E 向け文面。while ポーリングへの言及は while が実在するときだけ
                if f.has_while_loop {
                    smell.with_message(
                        "E2E でも分岐はテスト自体のバグの温床です。特に while による自前ポーリングは waitForExistence(timeout:) に置き換えられないか検討しましょう。",
                    )
                } else {
                    smell.with_message(
                        "E2E でも分岐はテスト自体のバグの温床です。分岐で結果の変わるテストは、条件ごとに別のテストへ分けましょう。",
                    )
                }
            })
            .collect()
    }
}
