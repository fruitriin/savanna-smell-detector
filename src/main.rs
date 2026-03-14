mod agent;
mod core;
mod detectors;
mod languages;
mod reporters;

use agent::{AgentTestSmell, load_rules, run_agent_detection};
use clap::Parser;
use languages::{LanguageParser, RustParser};
use reporters::{ConsoleReporter, MarkdownReporter, SmellReporter};
use serde::Serialize;
use std::path::PathBuf;
use std::io::Write as IoWrite;

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

    /// Additional magic number whitelist (comma-separated, e.g. "80,24,256")
    #[arg(long, value_delimiter = ',')]
    magic_number_whitelist: Vec<i64>,

    /// Minimum number of assertions without message to trigger Assertion Roulette (default: 2)
    #[arg(long, default_value_t = 2)]
    assertion_roulette_threshold: usize,

    /// Write report to file (extension determines format: .json → JSON, others → Markdown)
    #[arg(long)]
    output: Option<PathBuf>,

    /// Also print to stdout when --output is specified (tee mode)
    #[arg(long, default_value_t = false)]
    tee: bool,

    /// Show suppressed smells (by smell-allow comments)
    #[arg(long, default_value_t = false)]
    show_suppressed: bool,
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
    let registry = detectors::build_registry(cli.magic_number_whitelist.clone(), cli.assertion_roulette_threshold);
    let smells = registry.detect_all(&test_files);

    // smell-allow サプレスの適用
    let mut suppressed: Vec<core::SuppressedSmell> = Vec::new();
    let smells: Vec<_> = smells
        .into_iter()
        .filter(|s| s.smell_type.severity() >= cli.min_severity)
        .filter(|smell| {
            // ソースから smell-allow をパース（TestFile を探す）
            let test_file = test_files.iter().find(|tf| tf.path == smell.file_path);
            if let Some(tf) = test_file {
                if let Some(ref source) = tf.source {
                    let allows = core::parse_smell_allows(source);

                    // 関数レベルのスメル
                    if let Some(ref func_name) = smell.function_name {
                        if let Some(func) = tf.test_functions.iter().find(|f| f.name == *func_name) {
                            let func_allows = core::get_allows_for_function(&allows, func.line, func.body_line_count);
                            if func_allows.iter().any(|a| a.smell_types.contains(&smell.smell_type)) {
                                let reason = func_allows.iter()
                                    .find(|a| a.smell_types.contains(&smell.smell_type))
                                    .and_then(|a| a.reason.clone());
                                suppressed.push(core::SuppressedSmell {
                                    smell_type: smell.smell_type,
                                    file_path: smell.file_path.clone(),
                                    line: smell.line,
                                    function_name: smell.function_name.clone(),
                                    reason,
                                });
                                return false;
                            }
                        }
                    }

                    // ファイルレベルのスメル（CommentedOutTest, NoTest）
                    if core::is_line_suppressed(&allows, smell.smell_type, smell.line) {
                        suppressed.push(core::SuppressedSmell {
                            smell_type: smell.smell_type,
                            file_path: smell.file_path.clone(),
                            line: smell.line,
                            function_name: smell.function_name.clone(),
                            reason: allows.iter()
                                .find(|a| a.smell_types.contains(&smell.smell_type) && (a.line == smell.line || a.line + 1 == smell.line))
                                .and_then(|a| a.reason.clone()),
                        });
                        return false;
                    }
                }
            }
            true
        })
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
    if cli.min_severity > 1 && cli.format != "json" {
        eprintln!("  (showing severity >= {} only)", cli.min_severity);
    }

    // --output が指定されているかで出力先を決める
    if let Some(ref output_path) = cli.output {
        let ext = output_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");

        let content = if ext == "json" {
            // JSON出力
            let items: Vec<ReportItem> = smells
                .iter()
                .map(ReportItem::Ast)
                .chain(agent_smells.iter().map(ReportItem::Agent))
                .collect();
            serde_json::to_string_pretty(&items)
                .unwrap_or_else(|e| format!("{{\"error\": \"{}\"}}", e))
        } else {
            // Markdown出力
            let target = cli.path.display().to_string();
            let reporter = MarkdownReporter::new(target)
                .with_agent_smells(agent_smells.clone());
            reporter.report_all(&smells)
        };

        // ファイルに書き出す
        match std::fs::File::create(output_path) {
            Ok(mut f) => {
                if let Err(e) = f.write_all(content.as_bytes()) {
                    eprintln!("Failed to write output file: {}", e);
                }
            }
            Err(e) => {
                eprintln!("Failed to create output file {:?}: {}", output_path, e);
            }
        }

        // --tee なら stdout にも出力
        if cli.tee {
            println!("{}", content);
        }
    } else {
        // 通常の stdout 出力
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

                if !suppressed.is_empty() {
                    use colored::Colorize;
                    eprintln!(
                        "  ({} smell(s) suppressed by smell-allow — use --show-suppressed to list)",
                        suppressed.len()
                    );
                    if cli.show_suppressed {
                        for s in &suppressed {
                            let reason_str = s.reason.as_deref().unwrap_or("no reason given");
                            let location = match &s.function_name {
                                Some(name) => format!("{}:{} in {}", s.file_path, s.line, name),
                                None => format!("{}:{}", s.file_path, s.line),
                            };
                            eprintln!(
                                "    {} {} {}",
                                "[suppressed]".dimmed(),
                                s.smell_type,
                                location,
                            );
                            eprintln!("      📝 {}", reason_str.dimmed());
                        }
                    }
                }
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
