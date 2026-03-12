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

use crate::core::SmellDetectorRegistry;

/// Phase 1 + Phase 2 の全 Detector を登録済みの Registry を返す（デフォルト設定）
pub fn default_registry() -> SmellDetectorRegistry {
    build_registry(vec![])
}

/// ホワイトリスト等の設定を指定して Registry を構築する
pub fn build_registry(magic_number_extra_whitelist: Vec<i64>) -> SmellDetectorRegistry {
    SmellDetectorRegistry::new()
        .register(EmptyTestDetector)
        .register(MissingAssertionDetector)
        .register(SleepyTestDetector)
        .register(ConditionalLogicDetector)
        .register(IgnoredTestDetector)
        .register(RedundantPrintDetector)
        .register(AssertionRouletteDetector)
        .register(MagicNumberTestDetector::new().with_whitelist(magic_number_extra_whitelist))
        .register(SilentSkipDetector)
        .register(NoTestDetector)
        .register(FragileTestDetector)
}
