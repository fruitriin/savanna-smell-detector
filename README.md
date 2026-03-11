# savanna-smell-detector

Test smell detector for multiple languages — **Can you say the same in front of t_wada?**

Inspired by [savanna-maven-plugin](https://github.com/kawasima/savanna-maven-plugin).

## What is this?

A CLI tool that detects test code anti-patterns ("test smells") using AST analysis. Designed to integrate with CI pipelines and LLM-based auto-fix workflows.

```
🦁  6 test smell(s) detected:

  ███ Empty Test src/tests.rs:10 in test_empty
    💬 テストが空っぽですよ。それ、テストって呼べますか？

  ██░ Missing Assertion src/tests.rs:15 in test_no_assertion
    💬 アサーションがないテストは、テストではありません。ただの実行です。

  ██░ Sleepy Test src/tests.rs:22 in test_sleepy
    💬 sleep() をテストに書くのは、不安定さを自ら招いているようなものです。

  — t_wada の前でも同じこと言えんの？
```

## Features

- **Multi-language support** — Pluggable language parsers (Rust via `syn`, more coming)
- **AST-based detection** — Accurate analysis, not regex guessing
- **CI-friendly** — JSON output + `--fail-on-smell` exit code control
- **LLM-ready** — Structured JSON output that LLMs can read and act on
- **Extensible** — Adding a new smell is just implementing a trait

## Supported Languages

| Language | Parser | Status |
|----------|--------|--------|
| Rust | `syn` (AST) | Available |
| TypeScript | — | Planned |
| Python | — | Planned |
| Java | — | Planned |

## Detected Smells (Phase 1)

| Smell | Severity | Description |
|-------|----------|-------------|
| Empty Test | 5 | Test method with no body |
| Missing Assertion | 4 | Test without any assertions |
| Sleepy Test | 3 | Test using `sleep()` |
| Conditional Test Logic | 3 | `if`/`match`/`for`/`while` inside tests |
| Ignored Test | 2 | `#[ignore]`, `@Ignore`, `skip` etc. |
| Redundant Print | 1 | `println!`, `dbg!`, `console.log` left in tests |

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
```

### CI Integration Example (GitHub Actions)

```yaml
- name: Test smell check
  run: |
    savanna-smell-detector . -f json --fail-on-smell > smells.json
```

### LLM Auto-fix Workflow

```bash
# Detect smells → feed to LLM → auto-fix
savanna-smell-detector . -f json | llm "Fix these test smells"
```

## Adding New Smells

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

Then register it in the `SmellDetectorRegistry`.

## Architecture

```
src/
├── main.rs              # CLI entry point (clap)
├── core/                # Language-agnostic core
│   ├── smell.rs         # SmellType, TestSmell, TestFunction, TestFile
│   ├── detector.rs      # SmellDetector trait
│   └── registry.rs      # SmellDetectorRegistry
├── detectors/           # Smell detectors
│   ├── empty_test.rs
│   ├── missing_assertion.rs
│   ├── sleepy_test.rs
│   ├── conditional_logic.rs
│   ├── ignored_test.rs
│   └── redundant_print.rs
├── languages/           # Language parsers (extension point)
│   └── rust.rs          # Rust AST analysis via syn
└── reporters/           # Output formats
    ├── console.rs       # Colored console with severity bars
    └── json.rs          # Structured JSON for CI/LLM
```

## License

Apache License 2.0 — see [LICENSE](LICENSE).

## Acknowledgments

- [savanna-maven-plugin](https://github.com/kawasima/savanna-maven-plugin) by [@kawasima](https://github.com/kawasima) — the original inspiration
- The concept of "test smells" and the wisdom of [@t_wada](https://github.com/t-wada)
