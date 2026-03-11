mod core;
mod detectors;
mod languages;
mod reporters;

use clap::Parser;
use languages::{LanguageParser, RustParser};
use reporters::{ConsoleReporter, JsonReporter, SmellReporter};
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

    // 検出
    let registry = detectors::default_registry();
    let smells = registry.detect_all(&test_files);

    // レポート
    let reporter: Box<dyn SmellReporter> = match cli.format.as_str() {
        "json" => Box::new(JsonReporter),
        _ => Box::new(ConsoleReporter),
    };

    println!("{}", reporter.report(&smells));

    if cli.fail_on_smell && !smells.is_empty() {
        std::process::exit(1);
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
