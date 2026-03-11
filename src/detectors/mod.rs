mod empty_test;
mod missing_assertion;
mod sleepy_test;
mod conditional_logic;
mod ignored_test;
mod redundant_print;

pub use empty_test::EmptyTestDetector;
pub use missing_assertion::MissingAssertionDetector;
pub use sleepy_test::SleepyTestDetector;
pub use conditional_logic::ConditionalLogicDetector;
pub use ignored_test::IgnoredTestDetector;
pub use redundant_print::RedundantPrintDetector;

use crate::core::SmellDetectorRegistry;

/// Phase 1 の全 Detector を登録済みの Registry を返す
pub fn default_registry() -> SmellDetectorRegistry {
    SmellDetectorRegistry::new()
        .register(EmptyTestDetector)
        .register(MissingAssertionDetector)
        .register(SleepyTestDetector)
        .register(ConditionalLogicDetector)
        .register(IgnoredTestDetector)
        .register(RedundantPrintDetector)
}
