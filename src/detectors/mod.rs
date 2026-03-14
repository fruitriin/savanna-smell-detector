mod empty_test;
mod missing_assertion;
mod sleepy_test;
mod conditional_logic;
mod ignored_test;
mod redundant_print;
mod assertion_roulette;
mod magic_number;
mod no_test;
mod silent_skip;
mod fragile_test;
mod giant_test;
mod commented_out_test;

pub use empty_test::EmptyTestDetector;
pub use missing_assertion::MissingAssertionDetector;
pub use sleepy_test::SleepyTestDetector;
pub use conditional_logic::ConditionalLogicDetector;
pub use ignored_test::IgnoredTestDetector;
pub use redundant_print::RedundantPrintDetector;
pub use assertion_roulette::AssertionRouletteDetector;
pub use magic_number::MagicNumberTestDetector;
pub use no_test::NoTestDetector;
pub use silent_skip::SilentSkipDetector;
pub use fragile_test::FragileTestDetector;
pub use giant_test::GiantTestDetector;
pub use commented_out_test::CommentedOutTestDetector;

use crate::core::SmellDetectorRegistry;

/// Phase 1 + Phase 2 の全 Detector を登録済みの Registry を返す（デフォルト設定）
pub fn default_registry() -> SmellDetectorRegistry {
    build_registry(vec![], 2)
}

/// ホワイトリスト等の設定を指定して Registry を構築する
pub fn build_registry(magic_number_extra_whitelist: Vec<i64>, assertion_roulette_threshold: usize) -> SmellDetectorRegistry {
    SmellDetectorRegistry::new()
        .register(EmptyTestDetector)
        .register(MissingAssertionDetector)
        .register(SleepyTestDetector)
        .register(ConditionalLogicDetector)
        .register(IgnoredTestDetector)
        .register(RedundantPrintDetector)
        .register(AssertionRouletteDetector::new().with_threshold(assertion_roulette_threshold))
        .register(MagicNumberTestDetector::new().with_whitelist(magic_number_extra_whitelist))
        .register(SilentSkipDetector)
        .register(NoTestDetector)
        .register(FragileTestDetector)
        .register(GiantTestDetector::new())
        .register(CommentedOutTestDetector)
}
