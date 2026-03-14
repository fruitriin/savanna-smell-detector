# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
