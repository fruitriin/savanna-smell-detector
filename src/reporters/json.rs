use crate::core::TestSmell;
use super::SmellReporter;

pub struct JsonReporter;

impl SmellReporter for JsonReporter {
    fn report(&self, smells: &[TestSmell]) -> String {
        serde_json::to_string_pretty(smells).unwrap_or_else(|e| format!("{{\"error\": \"{}\"}}", e))
    }
}
