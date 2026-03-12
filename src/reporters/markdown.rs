use crate::agent::AgentTestSmell;
use crate::core::TestSmell;
use super::SmellReporter;
use chrono::Local;

pub struct MarkdownReporter {
    pub target: String,
    pub agent_smells: Vec<AgentTestSmell>,
}

impl MarkdownReporter {
    pub fn new(target: impl Into<String>) -> Self {
        Self {
            target: target.into(),
            agent_smells: Vec::new(),
        }
    }

    pub fn with_agent_smells(mut self, agent_smells: Vec<AgentTestSmell>) -> Self {
        self.agent_smells = agent_smells;
        self
    }

    pub fn report_all(&self, smells: &[TestSmell]) -> String {
        let now = Local::now();
        let generated = now.to_rfc3339();
        let ast_count = smells.len();
        let agent_count = self.agent_smells.len();
        let total = ast_count + agent_count;

        let mut out = String::new();

        out.push_str("# Savanna Smell Report\n\n");
        out.push_str(&format!("Generated: {}\n", generated));
        out.push_str(&format!("Target: {}\n", self.target));
        out.push_str(&format!(
            "Smells: {} (AST: {}, Agent: {})\n",
            total, ast_count, agent_count
        ));

        // AST Detection section
        out.push_str("\n## AST Detection\n\n");
        if smells.is_empty() {
            out.push_str("No AST smells detected.\n");
        } else {
            for smell in smells {
                let location = match &smell.function_name {
                    Some(name) => format!("`{}:{}` in {}", smell.file_path, smell.line, name),
                    None => format!("`{}:{}`", smell.file_path, smell.line),
                };
                out.push_str(&format!(
                    "- **{}** {} (severity: {})\n",
                    smell.smell_type,
                    location,
                    smell.smell_type.severity()
                ));
                out.push_str(&format!("  > {}\n\n", smell.message));
            }
        }

        // Agent Detection section
        out.push_str("## Agent Detection\n\n");
        if self.agent_smells.is_empty() {
            out.push_str("No Agent smells detected.\n");
        } else {
            for smell in &self.agent_smells {
                let confidence_pct = (smell.confidence * 100.0).round() as u32;
                out.push_str(&format!(
                    "- **{}** `{}:{}` in {} (confidence: {}%)\n",
                    smell.rule_name,
                    smell.file_path,
                    smell.line,
                    smell.function_name,
                    confidence_pct
                ));
                out.push_str(&format!("  > {}\n", smell.reason));
                if let Some(ref suggestion) = smell.suggestion {
                    out.push_str(&format!("  > 💡 {}\n", suggestion));
                }
                out.push('\n');
            }
        }

        out
    }
}

impl SmellReporter for MarkdownReporter {
    fn report(&self, smells: &[TestSmell]) -> String {
        self.report_all(smells)
    }
}
