use crate::core::TestSmell;
use super::SmellReporter;
use colored::Colorize;

pub struct ConsoleReporter;

impl SmellReporter for ConsoleReporter {
    fn report(&self, smells: &[TestSmell]) -> String {
        if smells.is_empty() {
            return format!(
                "\n{}  No test smells detected. Even t_wada would approve.\n",
                "✓".green()
            );
        }

        let mut output = String::new();
        output.push_str(&format!(
            "\n{}  {} test smell(s) detected:\n\n",
            "🦁".to_string(),
            smells.len()
        ));

        for smell in smells {
            let severity_indicator = match smell.smell_type.severity() {
                5 => "███".red().bold(),
                4 => "██░".red(),
                3 => "██░".yellow(),
                2 => "█░░".yellow(),
                _ => "░░░".white(),
            };

            let location = match &smell.function_name {
                Some(name) => format!("{}:{} in {}", smell.file_path, smell.line, name),
                None => format!("{}:{}", smell.file_path, smell.line),
            };

            output.push_str(&format!(
                "  {} {} {}\n",
                severity_indicator,
                smell.smell_type.to_string().bold(),
                location.dimmed()
            ));
            output.push_str(&format!(
                "    💬 {}\n\n",
                smell.message
            ));
        }

        output.push_str(&format!(
            "  {}\n",
            "— t_wada の前でも同じこと言えんの？".dimmed().italic()
        ));

        output
    }
}
