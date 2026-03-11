use crate::core::TestFunction;
use super::types::PrefilterConfig;

/// プレフィルターを適用し、LLMに送るべき対象かどうかを返す
/// true => LLMに送る候補
/// false => スキップ
pub fn apply_prefilter(config: &PrefilterConfig, func: &TestFunction) -> bool {
    if let Some(min) = config.min_assertions {
        if func.assertion_count < min {
            return false;
        }
    }

    if let Some(max) = config.max_assertions {
        if func.assertion_count > max {
            return false;
        }
    }

    if let Some(required) = config.has_conditional {
        if func.has_conditional != required {
            return false;
        }
    }

    if let Some(required) = config.has_sleep {
        if func.has_sleep != required {
            return false;
        }
    }

    if let Some(min_lines) = config.min_lines {
        let line_count = func.body_source.lines().count();
        if line_count < min_lines {
            return false;
        }
    }

    true
}
