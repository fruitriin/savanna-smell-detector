# savanna-smell-detector

Test smell detector for multiple languages — **Can you say the same in front of t_wada?**

Inspired by [savanna-maven-plugin](https://github.com/kawasima/savanna-maven-plugin).

> [日本語版はこちら](./README.ja.md)

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

- **Multi-language support** — Rust (`syn`), Go (`tree-sitter`), Python (`tree-sitter`), Swift (`tree-sitter`), Shell/Bash/Bats (regex) with `--language` filter
- **AST-based detection** — Accurate analysis with language-aware idiom recognition (e.g. Go's `if err != nil { t.Fatal }`)
- **LLM agent detection** — Optional Phase 2 detection using LLM-based rules
- **CI-friendly** — JSON output (with severity) + `--fail-on-smell` exit code control + severity filtering
- **Inline suppression** — `// smell-allow:` comments to suppress known intentional smells (works across all languages)
- **LLM-ready** — Structured JSON output that LLMs can read and act on
- **Extensible** — Adding a new smell is just implementing a trait, or writing a Markdown rule
- **Heuristic helpers** — Custom assertion helpers (`assertEqual`, `mustParse`, etc.) are recognized automatically

## Supported Languages

| Language | Parser | Status |
|----------|--------|--------|
| Rust | `syn` (AST) | Available |
| Shell / Bash / Bats | regex | Available |
| Go | tree-sitter (AST) | Available |
| Python | tree-sitter (AST) | Available |
| Swift | tree-sitter (AST) | Available |
| TypeScript | — | Planned |
| Java | — | Planned |

## Detected Smells

### Phase 1: AST-based Detection

| Smell | Severity | Rust | Shell | Go | Python | Swift | Description |
|-------|----------|------|-------|-----|--------|-------|-------------|
| Empty Test | 5 | ✅ | ✅ | ✅ | ✅ | ✅ | Test method with no body |
| No Test | 5 | ✅ | ✅ | ✅ | ✅ | ✅ | Source file with no test functions |
| Missing Assertion | 4 | ✅ | ✅ | ✅ | ✅ | ✅ | Test without any assertions (Go: `Benchmark*`/`Example*` excluded; custom helpers recognized) |
| Silent Skip | 4 | ✅ | ✅ | ✅ | ✅ | ✅ | Conditional early return that leaves the test (Swift: `guard ... else { return }` anywhere in the body) |
| Sleepy Test | 3 | ✅ | ✅ | ✅ | ✅ | ✅ | Test using `sleep()` / `time.Sleep()` / `time.sleep()` / `Thread.sleep` / `Task.sleep` |
| Conditional Test Logic | 3 | ✅ | ✅ | ✅ | ✅ | ✅ | `if`/`match`/`switch` branching (Go: `if cond { t.Fatal }` assertion idioms excluded) |
| Fragile Test | 3 | ✅ | ✅ | ✅ | ✅ | ✅ | Tests combining `sleep()` with time APIs (`Duration`/`timeout`/`context.WithTimeout`/etc.) |
| Giant Test | 3 | ✅ | ✅ | ✅ | ✅ | ✅ | Test function exceeding 50 lines (Go: table definitions excluded; Python: docstrings excluded) |
| Commented-Out Test | 3 | ✅ | ✅ | ✅ | ✅ | ✅ | Commented-out test markers (`// #[test]`, `// func TestXxx`, `# @test`, `# def test_xxx`, etc.) |
| Ignored Test | 2 | ✅ | ✅ | ✅ | ✅ | ✅ | `#[ignore]` / `skip` / `t.Skip()` / `@pytest.mark.skip` / `@Test(.disabled)` / `XCTSkip` |
| Assertion Roulette (Strict) | 2 | ✅ | — | — | — | ✅ | Multiple boolean-only assertions without messages (Rust: `assert!`; Swift: `XCTAssertTrue`/`XCTFail`) |
| Magic Number Test | 2 | ✅ | ✅ | ✅ | ✅ | ✅ | Unexplained numeric literals in assertions (whitelist: 0, 1, -1, 2 by default) |
| Assertion Roulette | 1 | ✅ | — | ✅ | ✅ | ✅ | Multiple value-showing assertions without messages (Swift: `#expect` / `XCTAssertEqual`) |
| Redundant Print | 1 | ✅ | ✅ | ✅ | ✅ | ✅ | Debug prints left in tests (`println!`/`fmt.Println`/`print()`/`NSLog`; Go: `t.Log` excluded — `-v` only) |

**Legend:** ✅ = implemented, — = not applicable or not yet implemented

### Go-specific Intelligence

The Go parser uses [tree-sitter](https://tree-sitter.github.io/) for accurate AST analysis and recognizes Go-specific patterns:

- **Assertion idiom exclusion** — `if condition { t.Fatal(...) }` is Go's standard assertion pattern (equivalent to Rust's `unwrap()`), not a test logic branch. These are excluded from Conditional Test Logic regardless of the condition (`err != nil`, `len(x) != N`, `!strings.Contains(...)`, etc.)
- **Custom helper recognition** — Functions named `assertEqual`, `mustParse`, `checkResult`, `expectError`, etc. are heuristically recognized as assertions
- **Table definition exclusion** — `[]struct{...}{...}` literal definitions (5+ lines) are excluded from Giant Test line counts
- **Benchmark/Example exclusion** — `Benchmark*` and `Example*` functions are excluded from Missing Assertion (benchmarks don't need assertions; examples use `// Output:` comments)
- **`t.Log` exclusion** — `t.Log`/`t.Logf` are excluded from Redundant Print (only shown with `go test -v`, unlike `fmt.Println`)

### Python-specific Intelligence

The Python parser uses tree-sitter and supports both **pytest** and **unittest** conventions:

- **Test discovery** — `def test_*` functions (module-level and class methods), including `async def`. Only files whose name contains `test` are scanned (`test_*.py`, `*_test.py`, `tests.py`, etc.)
- **Skip detection** — `@pytest.mark.skip(if)`, `@pytest.mark.xfail`, `@unittest.skip`, `self.skipTest(...)`, `pytest.skip(...)`, and class-level skip decorators (all methods inherit the skip)
- **Assertion recognition** — bare `assert`, `self.assert*`/`self.fail`, `pytest.raises`/`pytest.warns`, mock's `assert_called*`, and heuristic helpers (`assert_array_equal`, `check_*`, `verify_*`, etc.). `raise AssertionError` also counts
- **Assertion Roulette** — bare `assert x == y` without a `, "message"` counts as msgless (pytest shows values on failure, so severity stays at 1)
- **Docstring exclusion** — docstrings don't count toward Giant Test line counts, and a docstring-only test is still an Empty Test
- **Fragile timing** — `time.time()`/`time.monotonic()`/`datetime.now()` combined with `sleep` triggers Fragile Test

### Swift-specific Intelligence

The Swift parser uses tree-sitter and handles both **Swift Testing** (`@Test`) and **XCTest** conventions:

- **Attribute-based discovery** — Swift Testing tests are found by the `@Test` attribute, not by name. `@Test func nudgesAfterScheduledTime()` is a test even though it doesn't start with `test`. Works at top level and inside `struct`/`class`/`enum`/`extension`
- **XCTest discovery** — zero-argument `func test*` methods inside a type, only in files that actually use XCTest. Helper types living in the same file (`MockURLProtocol`, in-memory stores) are not mistaken for tests
- **Skip detection** — `@Test(.disabled(...))`, `@Suite(.disabled(...))` (inherited by every test in the suite), and `throw XCTSkip(...)`
- **Silent Skip** — `guard ... else { return }` anywhere in the body, not just at the top. `else { throw XCTSkip(...) }` and `{ XCTFail(...); return }` are correct and excluded. `guard` is not counted as Conditional Test Logic — in Swift it states a precondition rather than branching
- **Assertion tiering** — `#expect`/`#require` and `XCTAssertEqual` show values on failure (severity 1); `XCTAssertTrue`/`XCTFail` only show a boolean (severity 2)
- **Fragile timing** — a bare `Date()` used as fixture filler is ignored; it only counts when combined with time arithmetic (`addingTimeInterval`, `timeIntervalSince`) or explicit waits
- **Timeouts are not magic numbers** — `waitForExistence(timeout: 30)` is exempt: the argument label already says what the number means
- **XCUITest awareness** — files using `XCUIApplication` get E2E-specific wording. Waiting is unavoidable in E2E, so instead of a generic complaint the report names the fix (`waitForNonExistence(timeout:)` instead of a hand-rolled `Thread.sleep` polling loop)

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
# Scan target directory (default: ".")
target = "crates/"

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

# Language filter (rust, shell, go, python, swift)
language = "rust"
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

# Scan only Go test files
savanna-smell-detector . --language go

# Scan only Python test files
savanna-smell-detector . --language python

# Scan only Swift test files
savanna-smell-detector . --language swift

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
| `-l, --language` | — | Language filter: `rust`, `shell`, `go`, `python`, or `swift` (scan only the specified language) |

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
│   ├── rust.rs          # Rust AST analysis via syn
│   ├── shell.rs         # Shell/Bash/Bats regex-based analysis
│   ├── go.rs            # Go AST analysis via tree-sitter
│   ├── python.rs        # Python AST analysis via tree-sitter (pytest / unittest)
│   └── swift.rs         # Swift AST analysis via tree-sitter (Swift Testing / XCTest)
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
