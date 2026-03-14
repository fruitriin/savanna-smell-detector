mod agent;
mod config;
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

fn was_arg_provided(name: &str) -> bool {
    std::env::args().any(|a| a == format!("--{}", name) || a.starts_with(&format!("--{}=", name)))
}

fn main() {
    let cli = Cli::parse();

    // .savanna.toml からプロジェクト設定を読み込む
    // まず CWD から探す（target 未確定のため）。次に確定した path からも探す。
    let project_config = config::ProjectConfig::load(&std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

    // target のマージ: CLI位置引数がデフォルト "." なら config の target を使う
    let path = if cli.path == PathBuf::from(".") {
        project_config.target.as_ref()
            .map(|t| PathBuf::from(t))
            .unwrap_or(cli.path.clone())
    } else {
        cli.path.clone()
    };

    // マージ: CLI明示指定 > .savanna.toml > CLIデフォルト
    let min_severity = if was_arg_provided("min-severity") {
        cli.min_severity
    } else {
        project_config.min_severity.unwrap_or(cli.min_severity)
    };

    let fail_on_smell = if was_arg_provided("fail-on-smell") {
        cli.fail_on_smell
    } else {
        project_config.fail_on_smell.unwrap_or(cli.fail_on_smell)
    };

    let magic_number_whitelist = if was_arg_provided("magic-number-whitelist") {
        cli.magic_number_whitelist.clone()
    } else {
        project_config.magic_number_whitelist.unwrap_or(cli.magic_number_whitelist.clone())
    };

    let assertion_roulette_threshold = if was_arg_provided("assertion-roulette-threshold") {
        cli.assertion_roulette_threshold
    } else {
        project_config.assertion_roulette_threshold.unwrap_or(cli.assertion_roulette_threshold)
    };

    let agent_rules = if was_arg_provided("agent-rules") {
        cli.agent_rules.clone()
    } else {
        project_config.agent_rules.map(PathBuf::from).or(cli.agent_rules.clone())
    };

    let llm_command = if was_arg_provided("llm-command") {
        cli.llm_command.clone()
    } else {
        project_config.llm_command.unwrap_or(cli.llm_command.clone())
    };

    let agent_confidence = if was_arg_provided("agent-confidence") {
        cli.agent_confidence
    } else {
        project_config.agent_confidence.unwrap_or(cli.agent_confidence)
    };

    let glob_pattern = if was_arg_provided("glob") {
        cli.glob.clone()
    } else {
        project_config.glob.or(cli.glob.clone())
    };

    // 言語パーサーの登録
    let parsers: Vec<Box<dyn LanguageParser>> = vec![
        Box::new(RustParser),
    ];

    // ファイル収集
    let files = collect_files(&path, &glob_pattern, &parsers);

    if files.is_empty() {
        eprintln!("No test files found in {:?}", path);
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
    let registry = detectors::build_registry(magic_number_whitelist.clone(), assertion_roulette_threshold);
    let smells = registry.detect_all(&test_files);

    // smell-allow サプレスの適用
    let mut suppressed: Vec<core::SuppressedSmell> = Vec::new();
    let smells: Vec<_> = smells
        .into_iter()
        .filter(|s| s.smell_type.severity() >= min_severity)
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
        if let Some(ref rules_dir) = agent_rules {
            match load_rules(rules_dir) {
                Ok(rules) => {
                    if rules.is_empty() {
                        eprintln!("Warning: no agent rules found in {}", rules_dir.display());
                        Vec::new()
                    } else {
                        run_agent_detection(&rules, &test_files, &llm_command, agent_confidence)
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
    if min_severity > 1 && cli.format != "json" {
        eprintln!("  (showing severity >= {} only)", min_severity);
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
            let target = path.display().to_string();
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
    if fail_on_smell && has_smells {
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
