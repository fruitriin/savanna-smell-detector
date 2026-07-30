# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.5.0] - 2026-07-31

### Added

- **Swift 対応** — tree-sitter による Swift テストスメル検出
  - **Swift Testing / XCTest の両対応** — Swift Testing は `@Test` 属性で判定する（名前規約に依存しない）。トップレベル・`struct`/`class`/`enum`/`extension` のどこに置かれたテストも拾う
  - XCTest は型のボディ内の引数なし `func test*`。ただし XCTest を実際に使っているファイルのみを対象とし、同居するヘルパー型（モック等）をテストと誤認しない
  - スキップ検出: `@Test(.disabled(...))` / `@Suite(.disabled(...))`（配下の全テストに継承）/ `throw XCTSkip(...)`
  - アサーション認識: `#expect` / `#require` / `Issue.record` / `withKnownIssue` / `confirmation` / `XCTAssert*` / `XCTFail` / `XCTUnwrap`
  - アサーションの重み分け: 値を表示する `#expect`・`XCTAssertEqual` は Assertion Roulette（重要度1）、真偽値しか分からない `XCTAssertTrue`・`XCTFail` は Strict（重要度2）
  - Sleepy Test: `Thread.sleep` / `Task.sleep` / `usleep` / `RunLoop.current.run(until:)`
  - Commented-Out Test: `// @Test func ...` / `// func testXxx()` パターンを検出
- **`flavor` — テストの流儀に応じたメッセージ切り替え** — `TestFile` に `flavor` フィールドを追加。`XCUIApplication` を使うファイルは `xcuitest` と判定し、E2E 向けの文面に差し替える。E2E で待つこと自体は避けられないため、一般論で黙らせるのではなく直し方（`waitForNonExistence(timeout:)`）を名指しする。`flavor` 未指定時は言語名がキーになるため、言語固有の言い回し（Swift の Silent Skip では `#[ignore]` ではなく `@Test(.disabled(...))` / `XCTSkip` を案内）にも使える

### Changed

- **Silent Skip（Swift のみ）** — 「先頭3文以内の early return」ではなく本文全体を走査する。Swift の `guard ... else { return }` は位置に関係なく必ずそこで脱出するため。`else { throw XCTSkip(...) }` や `{ XCTFail(...); return }` は正しい対処として除外する
- **Magic Number（Swift のみ）** — `timeout` / `forTimeInterval` / `nanoseconds` / `seconds` / `until` / `deadline` ラベルの引数は対象外。引数ラベルが既にその数値の意味を説明しており、中身も仕様ではなく環境チューニングの値であるため
- **Conditional Test Logic（Swift のみ）** — `guard` は条件分岐に数えない。Swift の `guard` は分岐というより前提条件の表明であり、黙って脱出するものは Silent Skip 側で拾う

## [0.4.0] - 2026-07-05

### Added

- **Python 対応** — tree-sitter による Python テストスメル検出
  - pytest / unittest 両対応（`def test_*` 関数、`TestCase` メソッド、`async def` 含む）
  - スキップ検出: `@pytest.mark.skip(if)` / `@pytest.mark.xfail` / `@unittest.skip` / `self.skipTest` / `pytest.skip`（クラスレベルデコレーターは全メソッドに継承）
  - アサーション認識: 素の `assert`、`self.assert*`、`pytest.raises`/`warns`、mock の `assert_called*`、`raise AssertionError`、ヒューリスティックヘルパー
  - docstring を Giant Test の行数カウントから除外
  - Commented-Out Test: `# def test_xxx():` パターンを検出
- **Go 対応** (#15) — tree-sitter AST による Go テストスメル検出、`--language` オプション

### Fixed

- ディレクトリスキャンでテストファイルが1つも見つからない問題を修正 — glob クレートが `{rs,sh}` 形式のブレース展開に非対応のため、拡張子ごとにパターンを生成するように変更

## [0.3.0] - 2026-03-12

### Added

- **LLM Agent検出器** — markdownルールファイルによるオプトイン式テスト品質判定 (#1)
  - `--agent-rules <DIR>` でルールディレクトリを指定
  - `--llm-command` で LLM を切り替え可能（デフォルト: `claude -p`）。`ollama run` 等にも対応
  - `--agent-confidence` で信頼度閾値を設定
  - `--no-agent` で AST only モードに切り替え
  - デフォルト同梱ルール: `rules/t_wada.md`（総合レビュー）、`rules/eager-test.md`
- **SilentSkip 検出器** (severity 4) — `if ... { return; }` によるテスト無実行パターン (#4)
- **FragileTest 検出器** (severity 3) — `Duration::from_secs` / `Instant::now` 等のタイムアウト依存 (#6)
- **`--output <PATH>`** — 検出結果をファイルに書き出し。拡張子で形式を自動判定（`.json` → JSON、他 → Markdown）(#10)
- **`--tee`** — `--output` 指定時でも stdout にも出力 (#10)
- **Markdown レポーター** — AST 検出・Agent 検出を統合した読みやすいレポート形式 (#10)
- **`.savanna.toml` プロジェクト設定ファイル** — CLI 引数をファイルで管理 (#11)
  - `target` フィールドでスキャン対象パスを指定 (#13)
- **`// smell-allow: <SmellType>`** — インラインサプレス機構。特定行のスメルを抑制 (#12)
- **Magic Number ホワイトリスト** — デフォルト `[0, 1, -1, 2]` を除外。`--magic-number-whitelist` で追加指定可能 (#3)
- **`--min-severity`** — 指定 severity 未満のスメルを出力から除外。CI への段階的導入をサポート

### Changed

- **AssertionRoulette を severity 分岐** (#2)
  - `assert!` / `debug_assert!` → `AssertionRouletteStrict` (severity 2): メッセージなしでは診断不能
  - `assert_eq!` / `assert_ne!` → `AssertionRoulette` (severity 1): 自動差分表示があるため重大度を下げた

### Fixed

- `claude -p` が Claude Code セッション内でネスト検出エラーとなる問題を修正 (#9)
  - `CLAUDECODE` および `CLAUDE_CODE_ENTRYPOINT` 環境変数を LLM サブプロセス起動時に除去するように変更
- FragileTest: 状態構築ループ（`Duration` を使うが sleep しないパターン）の誤検知を修正 (#11)
- 実プロジェクト（SDIT）フィードバックに基づく検出精度の改善 (#11)

---

## [0.2.0] - (Unreleased)

### Added

- **AssertionRoulette 検出器** — メッセージなしの `assert!` 系マクロを検出 (severity 2)
- **MagicNumberTest 検出器** — テスト内の数値リテラルを検出 (severity 1)
- **NoTest 検出器** — テストが1件もないファイルを検出 (severity 1)

---

## [0.1.0] - (Unreleased)

### Added

- 初期リリース
- **Phase 1 検出器** (5種):
  - `IgnoredTest` — `#[ignore]` がついたテストを検出 (severity 2)
  - `SleepyTest` — `sleep()` を含むテストを検出 (severity 3)
  - `MissingAssertion` — アサーションがないテストを検出 (severity 5)
  - `ConditionalLogic` — テスト内の `if`/`match` 分岐を検出 (severity 2)
  - `RedundantPrint` — テスト内の `println!`/`eprintln!` を検出 (severity 1)
- Rust AST 解析（`syn` クレート）
- コンソール・JSON 出力形式
- `--fail-on-smell` フラグ（CI ゲートとして使用可能）
- `--format` / `-f` オプション（`console` / `json`）

[0.3.0]: https://github.com/fruitriin/savanna-smell-detector/releases/tag/v0.3.0
