// テスト臭いのサンプル集 — savanna-smell-detector の動作確認用

#[cfg(test)]
mod tests {
    use std::thread;
    use std::time::Duration;

    // 1. Empty Test — テストが空っぽ
    #[test]
    fn test_empty() {
    }

    // 2. Missing Assertion — アサーションなし
    #[test]
    fn test_no_assertion() {
        let x = 1 + 1;
        let _y = x * 2;
    }

    // 3. Sleepy Test — sleep を使っている
    #[test]
    fn test_sleepy() {
        thread::sleep(Duration::from_secs(2));
        assert_eq!(1, 1);
    }

    // 4. Conditional Test Logic — テスト内に条件分岐
    #[test]
    fn test_conditional() {
        let x = 42;
        if x > 0 {
            assert!(true);
        } else {
            assert!(false);
        }
    }

    // 5. Ignored Test — 無視されたテスト
    #[test]
    #[ignore]
    fn test_ignored() {
        assert_eq!(1, 1);
    }

    // 6. Redundant Print — printlnが残っている
    #[test]
    fn test_with_print() {
        println!("debug: calculating...");
        let result = 2 + 2;
        dbg!(result);
        assert_eq!(result, 4);
    }

    // 7. Assertion Roulette — メッセージなしの複数アサーション
    #[test]
    fn test_assertion_roulette() {
        let x = 42;
        assert!(x > 0);
        assert_eq!(x, 42);
        assert_ne!(x, 0);
    }

    // 8. Magic Number Test — マジックナンバー
    #[test]
    fn test_magic_number() {
        let result = compute();
        assert_eq!(result, 86400);
        assert_eq!(result % 3600, 0);
    }

    fn compute() -> i64 { 86400 }

    // 9. 良いテスト — 臭いなし
    #[test]
    fn test_clean() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}
