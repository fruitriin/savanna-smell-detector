# savanna-smell-detector

Test smell detector for multiple languages — **Can you say the same in front of t_wada?**

Inspired by [savanna-maven-plugin](https://github.com/kawasima/savanna-maven-plugin).

## What is this?

A CLI tool that detects test code anti-patterns ("test smells") using AST analysis and optional LLM-based detection. Designed to integrate with CI pipelines and LLM-based auto-fix workflows.

```
🦁  6 test smell(s) detected:

  ███ Empty Test src/tests.rs:10 in test_empty
    💬 テストが空っぽですよ。それ、テストって呼べますか？

  ██░ Missing Assertion src/tests.rs:15 in test_no_assertion
    💬 アサーションがないテストは、テストではありません。ただの実行です。

  ██░ Sleepy Test src/tests.rs:22 in test_sleepy
    💬 sleep() をテストに書くのは、不安定さを自ら招いているようなものです。

  — t_wada の前でも同じこと言えんの？

  (1 smell(s) suppressed by smell-allow — use --show-suppressed to list)
```

## Features

- **Multi-language support** — Pluggable language parsers (Rust via `syn`, more coming)
- **AST-based detection** — Accurate analysis, not regex guessing
- **LLM agent detection** — Optional Phase 2 detection using LLM-based rules
- **CI-friendly** — JSON output + `--fail-on-smell` exit code control + severity filtering
- **Inline suppression** — `// smell-allow:` comments to suppress known intentional smells
- **LLM-ready** — Structured JSON output that LLMs can read and act on
- **Extensible** — Adding a new smell is just implementing a trait, or writing a Markdown rule

## Supported Languages

| Language | Parser | Status |
|----------|--------|--------|
| Rust | `syn` (AST) | Available |
| TypeScript | — | Planned |
| Python | — | Planned |
| Java | — | Planned |

## Detected Smells

### Phase 1: AST-based Detection

| Smell | Severity | Description |
|-------|----------|-------------|
| Empty Test | 5 | Test method with no body |
| No Test | 5 | Source file with no test functions |
| Missing Assertion | 4 | Test without any assertions |
| Silent Skip | 4 | Conditional early return at the start of a test (`if !cond { return; }`) |
| Sleepy Test | 3 | Test using `sleep()` |
| Conditional Test Logic | 3 | `if`/`match` branching inside tests (table-driven `for` loops are excluded) |
| Fragile Test | 3 | Tests using `sleep()` with `Duration`/`Instant`/`SystemTime` APIs (time arithmetic without sleep is excluded) |
| Giant Test | 3 | Test function exceeding 50 lines (configurable) |
| Commented-Out Test | 3 | `// #[test]` commented-out test functions |
| Ignored Test | 2 | `#[ignore]` without a reason (reason-annotated `#[ignore = "..."]` is excluded) |
| Assertion Roulette (Strict) | 2 | Multiple `assert!` without messages (failure reason is completely unclear) |
| Magic Number Test | 2 | Unexplained numeric literals in assertions (whitelist: 0, 1, -1, 2 by default) |
| Assertion Roulette | 1 | Multiple `assert_eq!`/`assert_ne!` without messages (values are auto-displayed but intent is unclear) |
| Redundant Print | 1 | `println!`, `dbg!`, `eprintln!` left in tests |

### Phase 2: LLM Agent Detection (Optional)

Rules defined as Markdown files in a `rules/` directory. Included rules:

| Rule | Severity | Description |
|------|----------|-------------|
| Eager Test | 4 | Single test function verifying multiple independent behaviors |
| t_wada Review | — | Comprehensive t_wada-style test quality review |
| Env-Dependent Skip | 3 | `if !is_tty() { return; }` patterns that should use `#[ignore]` |
| Clone-and-Modify | 2 | Copy-pasted test code that should be parameterized |

## Installation

```bash
cargo install savanna-smell-detector
```

Or build from source:

```bash
git clone https://github.com/fruitriin/savanna-smell-detector.git
cd savanna-smell-detector
cargo build --release
```

## Project Configuration (`.savanna.toml`)

Create a `.savanna.toml` in your project root to persist CLI options:

```toml
# Minimum severity level (1-5)
min-severity = 1

# Fail on smell (for CI)
fail-on-smell = true

# Additional magic number whitelist
magic-number-whitelist = [24, 80, 255, 256, 4096]

# Assertion roulette threshold
assertion-roulette-threshold = 5

# Agent rules directory
agent-rules = "rules/"

# LLM command
llm-command = "claude -p"

# Agent confidence threshold
agent-confidence = 0.7

# File glob pattern
glob = "**/*.rs"
```

All fields are optional. CLI arguments override config file values. The config file is discovered by searching from the target directory upward to the filesystem root.

## Usage

```bash
# Scan current directory
savanna-smell-detector .

# Scan a specific file
savanna-smell-detector src/lib.rs

# JSON output (for CI / LLM consumption)
savanna-smell-detector . -f json

# Fail CI if smells found
savanna-smell-detector . --fail-on-smell

# Custom glob pattern
savanna-smell-detector . -g "**/*_test.rs"

# Filter by severity (only severity 3+)
savanna-smell-detector . --min-severity 3

# Extend magic number whitelist
savanna-smell-detector . --magic-number-whitelist "24,80,255,256"

# Adjust assertion roulette threshold
savanna-smell-detector . --assertion-roulette-threshold 4

# Show suppressed smells
savanna-smell-detector . --show-suppressed

# Write report to file (Markdown or JSON by extension)
savanna-smell-detector . --output report.md
savanna-smell-detector . --output report.json

# Tee mode: write to file AND stdout
savanna-smell-detector . --output report.md --tee

# LLM agent detection with custom rules
savanna-smell-detector . --agent-rules rules/
savanna-smell-detector . --agent-rules rules/ --llm-command "claude -p"
savanna-smell-detector . --agent-rules rules/ --agent-confidence 0.8

# Skip agent detection (AST-only)
savanna-smell-detector . --agent-rules rules/ --no-agent
```

### CLI Options

| Option | Default | Description |
|--------|---------|-------------|
| `<path>` | `.` | Target directory or file to scan |
| `-f, --format` | `console` | Output format: `console`, `json` |
| `-g, --glob` | — | File glob pattern (e.g. `"**/*_test.rs"`) |
| `--fail-on-smell` | `false` | Exit with code 1 if smells are found |
| `--min-severity` | `1` | Minimum severity level to report (1-5) |
| `--magic-number-whitelist` | — | Additional whitelisted numbers (comma-separated) |
| `--assertion-roulette-threshold` | `2` | Min assertions without message to trigger Assertion Roulette |
| `--show-suppressed` | `false` | Show smells suppressed by `smell-allow` comments |
| `--output` | — | Write report to file (`.json` → JSON, others → Markdown) |
| `--tee` | `false` | Also print to stdout when `--output` is specified |
| `--agent-rules` | — | Directory containing LLM agent rule files |
| `--llm-command` | `claude -p` | LLM command for agent detection |
| `--agent-confidence` | `0.7` | Minimum confidence threshold for agent results (0.0-1.0) |
| `--no-agent` | `false` | Skip agent rules (AST-only detection) |

## Inline Suppression (`smell-allow`)

Suppress specific smells with comments when the pattern is intentional:

```rust
// smell-allow: sleepy-test — Real process response wait, sleep is unavoidable
#[test]
fn test_pty_timeout() {
    thread::sleep(Duration::from_millis(100));
    assert!(pty.is_alive());
}

// smell-allow: silent-skip, conditional-test-logic — Environment-dependent test
#[test]
fn test_with_display() {
    if std::env::var("DISPLAY").is_err() { return; }
    // ...
}
```

**Scope rules:**
- Written before `#[test]` (within 5 lines) → applies to the entire function
- Written inside a test function → applies to the entire function
- Supports `—` (em dash) or `--` as reason separator
- Multiple smell types can be comma-separated

**Smell type names** (kebab-case): `empty-test`, `missing-assertion`, `sleepy-test`, `conditional-test-logic`, `ignored-test`, `redundant-print`, `assertion-roulette`, `assertion-roulette-strict`, `magic-number`, `no-test`, `silent-skip`, `fragile-test`, `giant-test`, `commented-out-test`

### CI Integration Example (GitHub Actions)

```yaml
- name: Test smell check (strict)
  run: |
    savanna-smell-detector . --min-severity 4 --fail-on-smell

- name: Test smell report (full)
  run: |
    savanna-smell-detector . --output smells.md --min-severity 2
```

### LLM Auto-fix Workflow

```bash
# Detect smells → feed to LLM → auto-fix
savanna-smell-detector . -f json | llm "Fix these test smells"
```

## Adding New Smells

### Option 1: AST Detector (Rust trait)

Implement the `SmellDetector` trait:

```rust
use savanna_smell_detector::core::{SmellDetector, SmellType, TestFile, TestSmell};

pub struct MyDetector;

impl SmellDetector for MyDetector {
    fn name(&self) -> &'static str { "MyDetector" }

    fn detect(&self, test_file: &TestFile) -> Vec<TestSmell> {
        // Your detection logic here
    }
}
```

Then register it in `src/detectors/mod.rs`.

### Option 2: LLM Agent Rule (Markdown)

Create a Markdown file in `rules/` with YAML frontmatter:

```markdown
---
name: "my-rule"
description: "Description of what this rule detects"
severity: 3
prefilter:
  min_assertions: 2
---

# My Rule

Your prompt template here. Use `{{input}}` for the test function JSON.

Return JSON: `{"is_smell": bool, "confidence": float, "reason": "...", "suggestion": "..."}`
```

## Architecture

```
src/
├── main.rs              # CLI entry point (clap)
├── config.rs            # .savanna.toml project config loader
├── core/                # Language-agnostic core
│   ├── smell.rs         # SmellType, TestSmell, TestFunction, TestFile
│   ├── smell_allow.rs   # Inline suppression (smell-allow comments)
│   ├── detector.rs      # SmellDetector trait
│   └── registry.rs      # SmellDetectorRegistry
├── detectors/           # AST-based smell detectors
│   ├── empty_test.rs
│   ├── missing_assertion.rs
│   ├── sleepy_test.rs
│   ├── conditional_logic.rs
│   ├── ignored_test.rs
│   ├── redundant_print.rs
│   ├── assertion_roulette.rs
│   ├── magic_number.rs
│   ├── silent_skip.rs
│   ├── fragile_test.rs
│   ├── giant_test.rs
│   ├── commented_out_test.rs
│   └── no_test.rs
├── agent/               # LLM agent detection (Phase 2)
│   ├── types.rs
│   ├── rule_loader.rs
│   ├── prefilter.rs
│   └── runner.rs
├── languages/           # Language parsers (extension point)
│   └── rust.rs          # Rust AST analysis via syn
└── reporters/           # Output formats
    ├── console.rs       # Colored console with severity bars
    ├── json.rs          # Structured JSON for CI/LLM
    └── markdown.rs      # Markdown report with timestamps
rules/                   # LLM agent rules (Markdown + YAML frontmatter)
├── eager-test.md
├── t_wada.md
├── env-dependent-skip.md
└── clone-and-modify.md
```

## License

Apache License 2.0 — see [LICENSE](LICENSE).

## Acknowledgments

- [savanna-maven-plugin](https://github.com/kawasima/savanna-maven-plugin) by [@kawasima](https://github.com/kawasima) — the original inspiration
- The concept of "test smells" and the wisdom of [@t_wada](https://github.com/t-wada)
