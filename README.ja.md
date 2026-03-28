# savanna-smell-detector

多言語対応テストスメル検出ツール — **t_wada の前でも同じこと言えんの？**

[savanna-maven-plugin](https://github.com/kawasima/savanna-maven-plugin) にインスパイアされています。

> [English version](./README.md)

## これは何？

テストコードのアンチパターン（「テストスメル」）を AST 解析で検出する CLI ツールです。CI パイプラインや LLM による自動修正ワークフローへの組み込みを想定しています。

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

## 特徴

- **多言語対応** — Rust (`syn`)、Go (`tree-sitter`)、Shell/Bash/Bats (regex)。`--language` でフィルタ可能
- **AST ベースの検出** — 言語固有のイディオムを理解した正確な解析（例: Go の `if err != nil { t.Fatal }` を除外）
- **LLM エージェント検出** — Markdown ルールによるオプショナルな Phase 2 検出
- **CI フレンドリー** — JSON 出力（severity 付き）+ `--fail-on-smell` + severity フィルタリング
- **インライン抑制** — `// smell-allow:` コメントで意図的なパターンを抑制（全言語対応）
- **LLM 連携** — 構造化 JSON 出力で LLM が読み取り・自動修正可能
- **拡張性** — trait 実装か Markdown ルール追加でスメルを追加
- **ヒューリスティック認識** — カスタムアサーションヘルパー（`assertEqual`、`mustParse` 等）を自動認識

## 対応言語

| 言語 | パーサー | 状態 |
|------|---------|------|
| Rust | `syn` (AST) | 対応済み |
| Shell / Bash / Bats | regex | 対応済み |
| Go | tree-sitter (AST) | 対応済み |
| TypeScript | — | 予定 |
| Python | — | 予定 |
| Java | — | 予定 |

## 検出するスメル

### Phase 1: AST ベース検出

| スメル | 重要度 | Rust | Shell | Go | 説明 |
|--------|--------|------|-------|-----|------|
| Empty Test | 5 | ✅ | ✅ | ✅ | ボディが空のテスト |
| No Test | 5 | ✅ | ✅ | ✅ | テスト関数がないファイル |
| Missing Assertion | 4 | ✅ | ✅ | ✅ | アサーションのないテスト（Go: `Benchmark*`/`Example*` 除外、カスタムヘルパー認識） |
| Silent Skip | 4 | ✅ | ✅ | ✅ | テスト先頭の条件付き early return |
| Sleepy Test | 3 | ✅ | ✅ | ✅ | `sleep()` / `time.Sleep()` の使用 |
| Conditional Test Logic | 3 | ✅ | ✅ | ✅ | `if`/`match`/`switch` 分岐（Go: `if cond { t.Fatal }` アサーションイディオムは除外） |
| Fragile Test | 3 | ✅ | ✅ | ✅ | `sleep()` と時間 API の併用（`Duration`/`timeout`/`context.WithTimeout` 等） |
| Giant Test | 3 | ✅ | ✅ | ✅ | 50行を超えるテスト関数（Go: テーブル定義は行数から除外） |
| Commented-Out Test | 3 | ✅ | ✅ | ✅ | コメントアウトされたテスト（`// #[test]`、`// func TestXxx`、`# @test` 等） |
| Ignored Test | 2 | ✅ | ✅ | ✅ | `#[ignore]` / `skip` / `t.Skip()` |
| Assertion Roulette (Strict) | 2 | ✅ | — | — | メッセージなし `assert!` の複数使用（Rust 固有: `assert!` vs `assert_eq!` の区別） |
| Magic Number Test | 2 | ✅ | ✅ | ✅ | アサーション内の説明なし数値リテラル（ホワイトリスト: 0, 1, -1, 2） |
| Assertion Roulette | 1 | ✅ | — | ✅ | メッセージなしアサーションの複数使用（Go: testify 引数カウントで判定） |
| Redundant Print | 1 | ✅ | ✅ | ✅ | テスト内のデバッグ出力（`println!`/`fmt.Println`。Go: `t.Log` は除外 — `-v` 時のみ表示） |

**凡例:** ✅ = 実装済み、— = 該当なし or 未実装

### Go 固有のインテリジェンス

Go パーサーは [tree-sitter](https://tree-sitter.github.io/) による正確な AST 解析を行い、Go 固有のパターンを認識します:

- **アサーションイディオム除外** — `if condition { t.Fatal(...) }` は Go の標準アサーションパターン（Rust の `unwrap()` に相当）。条件の内容を問わず（`err != nil`、`len(x) != N`、`!strings.Contains(...)` 等）Conditional Test Logic から除外
- **カスタムヘルパー認識** — `assertEqual`、`mustParse`、`checkResult`、`expectError` 等の関数名をヒューリスティックにアサーションとして認識
- **テーブル定義除外** — `[]struct{...}{...}` リテラル定義（5行以上）を Giant Test の行数カウントから除外
- **Benchmark/Example 除外** — `Benchmark*`/`Example*` 関数を Missing Assertion から除外（ベンチマークにアサーションは不要、Example は `// Output:` コメントが実質アサーション）
- **`t.Log` 除外** — `t.Log`/`t.Logf` を Redundant Print から除外（`go test -v` 時のみ表示で、`fmt.Println` とは別物）

### Phase 2: LLM エージェント検出（オプション）

`rules/` ディレクトリの Markdown ファイルでルールを定義。同梱ルール:

| ルール | 重要度 | 説明 |
|--------|--------|------|
| Eager Test | 4 | 1つのテスト関数で複数の独立した振る舞いを検証 |
| t_wada Review | — | t_wada 流の総合テスト品質レビュー |
| Env-Dependent Skip | 3 | `if !is_tty() { return; }` パターン（`#[ignore]` を使うべき） |
| Clone-and-Modify | 2 | コピペテスト（パラメータ化すべき） |

## インストール

```bash
cargo install savanna-smell-detector
```

ソースからビルド:

```bash
git clone https://github.com/fruitriin/savanna-smell-detector.git
cd savanna-smell-detector
cargo build --release
```

## プロジェクト設定（`.savanna.toml`）

プロジェクトルートに `.savanna.toml` を作成して CLI オプションを永続化:

```toml
# スキャン対象ディレクトリ（デフォルト: "."）
target = "crates/"

# 最小重要度レベル（1-5）
min-severity = 1

# スメル検出時に非ゼロ終了（CI用）
fail-on-smell = true

# マジックナンバーの追加ホワイトリスト
magic-number-whitelist = [24, 80, 255, 256, 4096]

# アサーションルーレットの閾値
assertion-roulette-threshold = 5

# Agent ルールディレクトリ
agent-rules = "rules/"

# LLM コマンド
llm-command = "claude -p"

# Agent 信頼度閾値
agent-confidence = 0.7

# ファイル glob パターン
glob = "**/*.rs"

# 言語フィルタ（rust, shell, go）
language = "rust"
```

全フィールドはオプション。CLI 引数が設定ファイルの値を上書きします。設定ファイルはスキャン対象ディレクトリから親ディレクトリを辿って検索されます。

## 使い方

```bash
# カレントディレクトリをスキャン
savanna-smell-detector .

# 特定のファイルをスキャン
savanna-smell-detector src/lib.rs

# JSON 出力（CI / LLM 連携用）
savanna-smell-detector . -f json

# スメル検出時に非ゼロ終了
savanna-smell-detector . --fail-on-smell

# glob パターン指定
savanna-smell-detector . -g "**/*_test.rs"

# 重要度フィルタ（3以上のみ）
savanna-smell-detector . --min-severity 3

# マジックナンバーホワイトリスト拡張
savanna-smell-detector . --magic-number-whitelist "24,80,255,256"

# アサーションルーレット閾値の調整
savanna-smell-detector . --assertion-roulette-threshold 4

# Go のテストファイルのみスキャン
savanna-smell-detector . --language go

# 抑制されたスメルを表示
savanna-smell-detector . --show-suppressed

# レポートをファイル出力（拡張子で形式を判定: .json → JSON、それ以外 → Markdown）
savanna-smell-detector . --output report.md
savanna-smell-detector . --output report.json

# tee モード: ファイルと stdout に同時出力
savanna-smell-detector . --output report.md --tee

# LLM エージェント検出
savanna-smell-detector . --agent-rules rules/
savanna-smell-detector . --agent-rules rules/ --llm-command "claude -p"
savanna-smell-detector . --agent-rules rules/ --agent-confidence 0.8

# Agent を無効化（AST のみ）
savanna-smell-detector . --agent-rules rules/ --no-agent
```

### CLI オプション

| オプション | デフォルト | 説明 |
|-----------|-----------|------|
| `<path>` | `.` | スキャン対象のディレクトリまたはファイル |
| `-f, --format` | `console` | 出力形式: `console`, `json` |
| `-g, --glob` | — | ファイル glob パターン（例: `"**/*_test.rs"`） |
| `--fail-on-smell` | `false` | スメル検出時に終了コード 1 で終了 |
| `--min-severity` | `1` | レポートする最小重要度レベル（1-5） |
| `--magic-number-whitelist` | — | 追加ホワイトリスト数値（カンマ区切り） |
| `--assertion-roulette-threshold` | `2` | Assertion Roulette の最小アサーション数 |
| `--show-suppressed` | `false` | `smell-allow` で抑制されたスメルを表示 |
| `--output` | — | レポートをファイルに出力（`.json` → JSON、それ以外 → Markdown） |
| `--tee` | `false` | `--output` 指定時に stdout にも出力 |
| `--agent-rules` | — | LLM Agent ルールファイルのディレクトリ |
| `--llm-command` | `claude -p` | Agent 検出に使う LLM コマンド |
| `--agent-confidence` | `0.7` | Agent 結果の最小信頼度閾値（0.0-1.0） |
| `--no-agent` | `false` | Agent ルールをスキップ（AST のみ） |
| `-l, --language` | — | 言語フィルタ: `rust`, `shell`, `go` |

## インライン抑制（`smell-allow`）

意図的なパターンの場合、コメントでスメルを抑制できます:

```rust
// smell-allow: sleepy-test — 実プロセスの応答待ちで sleep が不可避
#[test]
fn test_pty_timeout() {
    thread::sleep(Duration::from_millis(100));
    assert!(pty.is_alive());
}

// smell-allow: silent-skip, conditional-test-logic — 環境依存テスト
#[test]
fn test_with_display() {
    if std::env::var("DISPLAY").is_err() { return; }
    // ...
}
```

**スコープルール:**
- `#[test]` の前（5行以内）に書くと関数全体に適用
- テスト関数内に書いても関数全体に適用
- `—`（em ダッシュ）または `--` で理由を記述可能
- 複数のスメルタイプをカンマ区切りで指定可能

**スメルタイプ名**（kebab-case）: `empty-test`, `missing-assertion`, `sleepy-test`, `conditional-test-logic`, `ignored-test`, `redundant-print`, `assertion-roulette`, `assertion-roulette-strict`, `magic-number`, `no-test`, `silent-skip`, `fragile-test`, `giant-test`, `commented-out-test`

### CI 統合例（GitHub Actions）

```yaml
- name: テストスメルチェック（厳格）
  run: |
    savanna-smell-detector . --min-severity 4 --fail-on-smell

- name: テストスメルレポート（全件）
  run: |
    savanna-smell-detector . --output smells.md --min-severity 2
```

### LLM 自動修正ワークフロー

```bash
# スメル検出 → LLM に渡す → 自動修正
savanna-smell-detector . -f json | llm "Fix these test smells"
```

## スメルの追加方法

### 方法 1: AST 検出器（Rust trait）

`SmellDetector` trait を実装:

```rust
use savanna_smell_detector::core::{SmellDetector, SmellType, TestFile, TestSmell};

pub struct MyDetector;

impl SmellDetector for MyDetector {
    fn name(&self) -> &'static str { "MyDetector" }

    fn detect(&self, test_file: &TestFile) -> Vec<TestSmell> {
        // 検出ロジック
    }
}
```

`src/detectors/mod.rs` に登録。

### 方法 2: LLM Agent ルール（Markdown）

`rules/` に YAML フロントマター付き Markdown ファイルを追加:

```markdown
---
name: "my-rule"
description: "検出ルールの説明"
severity: 3
prefilter:
  min_assertions: 2
---

# My Rule

プロンプトテンプレート。`{{input}}` にテスト関数の JSON が埋め込まれます。

LLM は `{"is_smell": bool, "confidence": float, "reason": "...", "suggestion": "..."}` を返してください。
```

## アーキテクチャ

```
src/
├── main.rs              # CLI エントリポイント (clap)
├── config.rs            # .savanna.toml プロジェクト設定ローダー
├── core/                # 言語非依存のコア
│   ├── smell.rs         # SmellType, TestSmell, TestFunction, TestFile
│   ├── smell_allow.rs   # インライン抑制（smell-allow コメント）
│   ├── detector.rs      # SmellDetector trait
│   └── registry.rs      # SmellDetectorRegistry
├── detectors/           # AST ベースのスメル検出器
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
├── agent/               # LLM エージェント検出 (Phase 2)
│   ├── types.rs
│   ├── rule_loader.rs
│   ├── prefilter.rs
│   └── runner.rs
├── languages/           # 言語パーサー（拡張ポイント）
│   ├── rust.rs          # Rust AST 解析 (syn)
│   ├── shell.rs         # Shell/Bash/Bats regex ベース解析
│   └── go.rs            # Go AST 解析 (tree-sitter)
└── reporters/           # 出力形式
    ├── console.rs       # 色付きコンソール + severity バー
    ├── json.rs          # CI/LLM 連携用 JSON
    └── markdown.rs      # タイムスタンプ付き Markdown レポート
rules/                   # LLM Agent ルール（Markdown + YAML フロントマター）
├── eager-test.md
├── t_wada.md
├── env-dependent-skip.md
└── clone-and-modify.md
```

## ライセンス

Apache License 2.0 — [LICENSE](LICENSE) を参照。

## 謝辞

- [savanna-maven-plugin](https://github.com/kawasima/savanna-maven-plugin) by [@kawasima](https://github.com/kawasima) — オリジナルの着想元
- 「テストスメル」の概念と [@t_wada](https://github.com/t-wada) の知恵
