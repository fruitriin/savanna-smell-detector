use super::SmellType;

/// ファイル内の smell-allow ディレクティブ
#[derive(Debug, Clone)]
pub struct SmellAllow {
    pub line: usize,  // 1-indexed
    pub smell_types: Vec<SmellType>,
    pub reason: Option<String>,
}

/// サプレスされたスメルの情報
#[derive(Debug, Clone, serde::Serialize)]
pub struct SuppressedSmell {
    pub smell_type: SmellType,
    pub file_path: String,
    pub line: usize,
    pub function_name: Option<String>,
    pub reason: Option<String>,
}

/// ソースコードから smell-allow ディレクティブをパースする
pub fn parse_smell_allows(source: &str) -> Vec<SmellAllow> {
    let mut allows = Vec::new();
    for (i, line) in source.lines().enumerate() {
        let line_num = i + 1;
        // コメント中の "smell-allow:" を探す
        if let Some(pos) = line.find("smell-allow:") {
            let after = &line[pos + "smell-allow:".len()..];
            // "— 理由" or "-- 理由" で分割
            let (types_part, reason) = if let Some(idx) = after.find('—') {
                (&after[..idx], Some(after[idx + '—'.len_utf8()..].trim().to_string()))
            } else if let Some(idx) = after.find("--") {
                (&after[..idx], Some(after[idx + 2..].trim().to_string()))
            } else {
                (after.trim_end(), None)
            };

            let smell_types: Vec<SmellType> = types_part
                .split(',')
                .filter_map(|s| SmellType::from_kebab_name(s.trim()))
                .collect();

            if !smell_types.is_empty() {
                // reason が空文字列の場合は None にする
                let reason = reason.filter(|r| !r.is_empty());
                allows.push(SmellAllow {
                    line: line_num,
                    smell_types,
                    reason,
                });
            }
        }
    }
    allows
}

/// テスト関数に対応する smell-allow を返す
/// - 関数の行番号の直前（1-5行以内）に書かれた allow → 関数全体に適用
/// - 関数内（line..line+body_lines）に書かれた allow → 関数に適用
pub fn get_allows_for_function(
    allows: &[SmellAllow],
    func_line: usize,
    body_line_count: usize,
) -> Vec<&SmellAllow> {
    allows
        .iter()
        .filter(|a| {
            // 関数直前（5行以内）
            if a.line < func_line && func_line - a.line <= 5 {
                return true;
            }
            // 関数内
            if a.line >= func_line && a.line <= func_line + body_line_count {
                return true;
            }
            false
        })
        .collect()
}

/// 行番号ベースのサプレス（ファイルレベルスメル用）
pub fn is_line_suppressed(allows: &[SmellAllow], smell_type: SmellType, line: usize) -> bool {
    allows.iter().any(|a| {
        // 同一行 or 直前行
        (a.line == line || a.line + 1 == line) && a.smell_types.contains(&smell_type)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::SmellType;

    #[test]
    fn test_parse_smell_allows_basic() {
        let source = "// smell-allow: sleepy-test — 実プロセスの応答待ち\n#[test]\nfn foo() {}";
        let allows = parse_smell_allows(source);
        assert_eq!(allows.len(), 1);
        assert_eq!(allows[0].line, 1);
        assert_eq!(allows[0].smell_types, vec![SmellType::SleepyTest]);
        assert_eq!(allows[0].reason.as_deref(), Some("実プロセスの応答待ち"));
    }

    #[test]
    fn test_parse_smell_allows_multiple_types() {
        let source = "// smell-allow: magic-number, sleepy-test -- 複数";
        let allows = parse_smell_allows(source);
        assert_eq!(allows.len(), 1);
        assert_eq!(allows[0].smell_types.len(), 2);
    }

    #[test]
    fn test_parse_smell_allows_no_reason() {
        let source = "// smell-allow: empty-test";
        let allows = parse_smell_allows(source);
        assert_eq!(allows.len(), 1);
        assert!(allows[0].reason.is_none());
    }

    #[test]
    fn test_get_allows_for_function() {
        let allows = vec![
            SmellAllow {
                line: 2,
                smell_types: vec![SmellType::SleepyTest],
                reason: None,
            },
        ];
        // func_line=3 → 直前2行以内
        let result = get_allows_for_function(&allows, 3, 5);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_is_line_suppressed() {
        let allows = vec![
            SmellAllow {
                line: 5,
                smell_types: vec![SmellType::CommentedOutTest],
                reason: None,
            },
        ];
        assert!(is_line_suppressed(&allows, SmellType::CommentedOutTest, 5));
        assert!(is_line_suppressed(&allows, SmellType::CommentedOutTest, 6));
        assert!(!is_line_suppressed(&allows, SmellType::CommentedOutTest, 7));
    }
}
