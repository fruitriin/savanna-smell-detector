mod agent;
mod core;
mod detectors;
mod languages;
mod reporters;

use agent::{AgentTestSmell, load_rules, run_agent_detection};
use clap::Parser;
use languages::{LanguageParser, RustParser};
use reporters::{ConsoleReporter, SmellReporter};
use serde::Serialize;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "savanna-smell-detector")]
#[command(about = "Test smell detector — Can you say the same in front of t_wada?")]
#[command(version)]
struct Cli {
    /// Target directory or file to scan
    #[arg(default_value = ".")]
    path: PathBuf,

    /// Output format: console, json
    #[arg(short, long, default_value = "console")]
    format: String,

    /// File glob pattern (e.g. "**/*_test.rs")
    #[arg(short, long)]
    glob: Option<String>,

    /// Fail with non-zero exit code if smells are found
    #[arg(long, default_value_t = false)]
    fail_on_smell: bool,

    /// Minimum severity level to report (1-5, default: 1)
    #[arg(long, default_value_t = 1)]
    min_severity: u8,

    /// Agent-based detection rules directory
    #[arg(long)]
    agent_rules: Option<PathBuf>,

    /// LLM command to use (default: "claude -p")
    #[arg(long, default_value = "claude -p")]
    llm_command: String,

    /// Minimum confidence threshold for agent results (0.0-1.0)
    #[arg(long, default_value_t = 0.7)]
    agent_confidence: f64,

    /// Skip agent rules (run AST-only detection)
    #[arg(long, default_value_t = false)]
    no_agent: bool,
}

/// 統合出力用のアイテム型
#[derive(Serialize)]
#[serde(tag = "detector")]
enum ReportItem<'a> {
    #[serde(rename = "ast")]
    Ast(&'a crate::core::TestSmell),
    #[serde(rename = "agent")]
    Agent(&'a AgentTestSmell),
}

fn main() {
    let cli = Cli::parse();

    // 言語パーサーの登録
    let parsers: Vec<Box<dyn LanguageParser>> = vec![
        Box::new(RustParser),
    ];

    // ファイル収集
    let files = collect_files(&cli.path, &cli.glob, &parsers);

    if files.is_empty() {
        eprintln!("No test files found in {:?}", cli.path);
        std::process::exit(0);
    }

    // パース
    let mut test_files = Vec::new();
    for (path, parser_idx) in &files {
        let source = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Failed to read {}: {}", path.display(), e);
                continue;
            }
        };
        if let Some(test_file) = parsers[*parser_idx].parse(&path.to_string_lossy(), &source) {
            if !test_file.test_functions.is_empty() {
                test_files.push(test_file);
            }
        }
    }

    // Phase 1: AST検出
    let registry = detectors::default_registry();
    let smells = registry.detect_all(&test_files);

    // フィルタリング
    let smells: Vec<_> = smells
        .into_iter()
        .filter(|s| s.smell_type.severity() >= cli.min_severity)
        .collect();

    // Phase 2: Agent検出（--agent-rules が指定され、--no-agent でなければ）
    let agent_smells: Vec<AgentTestSmell> = if !cli.no_agent {
        if let Some(ref rules_dir) = cli.agent_rules {
            match load_rules(rules_dir) {
                Ok(rules) => {
                    if rules.is_empty() {
                        eprintln!("Warning: no agent rules found in {}", rules_dir.display());
                        Vec::new()
                    } else {
                        run_agent_detection(&rules, &test_files, &cli.llm_command, cli.agent_confidence)
                    }
                }
                Err(e) => {
                    eprintln!("Warning: failed to load agent rules — {}", e);
                    Vec::new()
                }
            }
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    };

    // レポート出力
    if cli.min_severity > 1 {
        println!("  (showing severity >= {} only)", cli.min_severity);
    }

    match cli.format.as_str() {
        "json" => {
            // 統合JSON出力
            let items: Vec<ReportItem> = smells
                .iter()
                .map(ReportItem::Ast)
                .chain(agent_smells.iter().map(ReportItem::Agent))
                .collect();
            println!("{}", serde_json::to_string_pretty(&items)
                .unwrap_or_else(|e| format!("{{\"error\": \"{}\"}}", e)));
        }
        _ => {
            // コンソール出力: Phase 1 → Phase 2
            let reporter: Box<dyn SmellReporter> = Box::new(ConsoleReporter);
            println!("{}", reporter.report(&smells));

            if !agent_smells.is_empty() {
                print_agent_smells_console(&agent_smells);
            }
        }
    }

    let has_smells = !smells.is_empty() || !agent_smells.is_empty();
    if cli.fail_on_smell && has_smells {
        std::process::exit(1);
    }
}

fn print_agent_smells_console(agent_smells: &[AgentTestSmell]) {
    use colored::Colorize;

    println!(
        "\n{}  {} agent smell(s) detected:\n",
        "🤖".to_string(),
        agent_smells.len()
    );

    for smell in agent_smells {
        let severity_indicator = match smell.severity {
            5 => "███".red().bold(),
            4 => "██░".red(),
            3 => "██░".yellow(),
            2 => "█░░".yellow(),
            _ => "░░░".white(),
        };

        let confidence_str = format!("(confidence: {:.0}%)", smell.confidence * 100.0);

        println!(
            "  {} {} {}:{} in {} {}",
            severity_indicator,
            smell.rule_name.bold(),
            smell.file_path,
            smell.line,
            smell.function_name,
            confidence_str.dimmed()
        );
        println!("    💬 {}", smell.reason);
        if let Some(ref suggestion) = smell.suggestion {
            println!("    💡 {}", suggestion.dimmed());
        }
        println!();
    }
}

fn collect_files(
    path: &PathBuf,
    glob_pattern: &Option<String>,
    parsers: &[Box<dyn LanguageParser>],
) -> Vec<(PathBuf, usize)> {
    let mut results = Vec::new();

    let pattern = if let Some(g) = glob_pattern {
        if path.is_dir() {
            format!("{}/{}", path.display(), g)
        } else {
            g.clone()
        }
    } else if path.is_file() {
        return find_parser_for_file(path, parsers)
            .map(|idx| vec![(path.clone(), idx)])
            .unwrap_or_default();
    } else {
        format!("{}/**/*.rs", path.display())
    };

    if let Ok(entries) = glob::glob(&pattern) {
        for entry in entries.flatten() {
            if let Some(idx) = find_parser_for_file(&entry, parsers) {
                results.push((entry, idx));
            }
        }
    }

    results
}

fn find_parser_for_file(path: &PathBuf, parsers: &[Box<dyn LanguageParser>]) -> Option<usize> {
    let ext = path.extension()?.to_str()?;
    parsers
        .iter()
        .position(|p| p.extensions().contains(&ext))
}
