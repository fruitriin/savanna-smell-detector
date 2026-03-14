use serde::Serialize;
use std::fmt;

/// テスト臭いの種類
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SmellType {
    /// テストメソッドに処理がない
    EmptyTest,
    /// アサーション文がないテスト
    MissingAssertion,
    /// sleep() を使用しているテスト
    SleepyTest,
    /// テスト内の条件分岐 (if/for/while/match)
    ConditionalTestLogic,
    /// 無視されたテスト (#[ignore], @Ignore, skip, etc.)
    IgnoredTest,
    /// 不要な print/println/dbg! 文
    RedundantPrint,
    /// メッセージなしの複数アサーション（assert_eq!/assert_ne! — 値が自動表示される）
    AssertionRoulette,
    /// メッセージなしの複数アサーション（assert! のみ — 失敗理由が不明）
    AssertionRouletteStrict,
    /// 説明のないマジックナンバー
    MagicNumberTest,
    /// テストがない
    NoTest,
    /// テスト関数の先頭付近に条件付き early return がある
    SilentSkip,
    /// タイムアウト依存の成功判定（Duration/Instant/SystemTime の使用）
    FragileTest,
    /// テスト関数が長すぎる（複数の関心事を持つ兆候）
    GiantTest,
    /// コメントアウトされたテスト関数
    CommentedOutTest,
}

impl SmellType {
    /// t_wada が言いそうなメッセージ
    pub fn roar(&self) -> &'static str {
        match self {
            SmellType::EmptyTest =>
                "テストが空っぽですよ。それ、テストって呼べますか？",
            SmellType::MissingAssertion =>
                "アサーションがないテストは、テストではありません。ただの実行です。",
            SmellType::SleepyTest =>
                "sleep() をテストに書くのは、不安定さを自ら招いているようなものです。",
            SmellType::ConditionalTestLogic =>
                "テストの中に条件分岐があるということは、テスト自体にバグが入る余地があるということです。",
            SmellType::IgnoredTest =>
                "無視されたテストは、壊れた窓と同じです。いつか直すつもりなら、今直しましょう。",
            SmellType::RedundantPrint =>
                "print デバッグをテストに残すのは、作業中の足場を建物に残すようなものです。",
            SmellType::AssertionRoulette =>
                "assert_eq!/assert_ne! は失敗時に値を表示しますが、メッセージがあるとさらに意図が明確になります。",
            SmellType::AssertionRouletteStrict =>
                "assert! にメッセージがないと、失敗したとき何が期待と違ったのか全く分かりません。",
            SmellType::MagicNumberTest =>
                "その数値は何を意味していますか？テストは仕様の表明です。意図を名前にしましょう。",
            SmellType::NoTest =>
                "テストがありませんね。t_wada の前でも同じこと言えんの？",
            SmellType::SilentSkip =>
                "テストが通ったんじゃない、テストが実行されなかっただけだ。条件付きスキップは #[ignore] を使いましょう。",
            SmellType::FragileTest =>
                "固定タイムアウトに頼るテストは、CI の負荷が高い日に裏切ります。時間ではなくイベントを待ちましょう。",
            SmellType::GiantTest =>
                "テスト関数が長すぎます。1つのテストは1つの関心事を検証すべきです。分割を検討しましょう。",
            SmellType::CommentedOutTest =>
                "コメントアウトされたテストは、削除するか有効にするか、どちらかにしましょう。中途半端が一番よくない。",
        }
    }

    /// 重要度 (1-5)
    pub fn severity(&self) -> u8 {
        match self {
            SmellType::EmptyTest => 5,
            SmellType::MissingAssertion => 4,
            SmellType::SleepyTest => 3,
            SmellType::ConditionalTestLogic => 3,
            SmellType::IgnoredTest => 2,
            SmellType::RedundantPrint => 1,
            SmellType::AssertionRoulette => 1,
            SmellType::AssertionRouletteStrict => 2,
            SmellType::MagicNumberTest => 2,
            SmellType::NoTest => 5,
            SmellType::SilentSkip => 4,
            SmellType::FragileTest => 3,
            SmellType::GiantTest => 3,
            SmellType::CommentedOutTest => 3,
        }
    }
}

impl fmt::Display for SmellType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            SmellType::EmptyTest => "Empty Test",
            SmellType::MissingAssertion => "Missing Assertion",
            SmellType::SleepyTest => "Sleepy Test",
            SmellType::ConditionalTestLogic => "Conditional Test Logic",
            SmellType::IgnoredTest => "Ignored Test",
            SmellType::RedundantPrint => "Redundant Print",
            SmellType::AssertionRoulette => "Assertion Roulette",
            SmellType::AssertionRouletteStrict => "Assertion Roulette (Strict)",
            SmellType::MagicNumberTest => "Magic Number Test",
            SmellType::NoTest => "No Test",
            SmellType::SilentSkip => "Silent Skip",
            SmellType::FragileTest => "Fragile Test",
            SmellType::GiantTest => "Giant Test",
            SmellType::CommentedOutTest => "Commented-Out Test",
        };
        write!(f, "{}", s)
    }
}

/// 検出されたテスト臭い
#[derive(Debug, Clone, Serialize)]
pub struct TestSmell {
    pub smell_type: SmellType,
    pub file_path: String,
    pub line: usize,
    pub function_name: Option<String>,
    pub message: String,
}

impl TestSmell {
    pub fn new(
        smell_type: SmellType,
        file_path: impl Into<String>,
        line: usize,
        function_name: Option<String>,
    ) -> Self {
        Self {
            message: smell_type.roar().to_string(),
            smell_type,
            file_path: file_path.into(),
            line,
            function_name,
        }
    }
}

/// 検出対象のテスト関数情報（言語非依存）
#[derive(Debug, Clone)]
pub struct TestFunction {
    pub name: String,
    pub line: usize,
    pub body_source: String,
    pub is_ignored: bool,
    pub has_assertion: bool,
    pub has_sleep: bool,
    pub has_conditional: bool,
    /// 真の条件分岐 (if/match) があるか（for/while/loop は含まない）
    pub has_branching: bool,
    /// for ループがあるか
    pub has_for_loop: bool,
    /// for ループ内にアサーションがあるか（テーブル駆動テストの兆候）
    pub has_assertion_in_loop: bool,
    pub has_print: bool,
    pub is_empty: bool,
    pub assertion_count: usize,
    /// assert! のみのカウント（assert_eq!/assert_ne! を除く）
    pub assert_only_count: usize,
    /// メッセージなし assert_eq!/assert_ne! のカウント
    pub assertions_without_message: usize,
    /// メッセージなし assert!/debug_assert! のカウント
    pub assert_only_without_message: usize,
    pub magic_numbers: Vec<(i64, usize)>, // (value, line)
    /// テスト関数の先頭付近（最初の3文）に条件付き early return があるか
    pub has_early_return: bool,
    /// Duration::from_secs/from_millis, Instant::now(), SystemTime::now() などの時間API使用（sleep以外）
    pub has_timeout_dependency: bool,
    /// テスト関数のボディの行数
    pub body_line_count: usize,
}

/// ファイル単位の解析結果（言語非依存）
#[derive(Debug, Clone)]
pub struct TestFile {
    pub path: String,
    pub language: String,
    pub test_functions: Vec<TestFunction>,
    /// ソースコードの全文（Commented-Out Test 検出等で使用）
    pub source: Option<String>,
}
