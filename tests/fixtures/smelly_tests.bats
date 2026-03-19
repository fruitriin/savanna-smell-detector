#!/usr/bin/env bats
# テスト臭いのサンプル集 — Shell/Bats 版

# 1. Empty Test — テストが空っぽ
@test "empty test" {
}

# 2. Missing Assertion — アサーションなし
@test "no assertion" {
    result=$((2 + 2))
    _unused="$result"
}

# 3. Sleepy Test — sleep を使っている
@test "sleepy test" {
    sleep 2
    [ "$status" -eq 0 ]
}

# 4. Conditional Test Logic — テスト内に条件分岐
@test "conditional logic" {
    result=$((2 + 2))
    if [ "$result" -eq 4 ]; then
        assert_success
    else
        fail "unexpected"
    fi
}

# 5. Ignored Test — skip で無視
@test "skipped test" {
    skip "not implemented yet"
    [ 1 -eq 1 ]
}

# 6. Redundant Print — echo が残っている
@test "with debug echo" {
    echo "debug: starting test"
    result=$((2 + 2))
    [ "$result" -eq 4 ]
}

# 7. 良いテスト — 臭いなし
@test "clean test" {
    result=$((2 + 2))
    [ "$result" -eq 4 ]
}

# 8. Giant Test — 長すぎるテスト
@test "giant test" {
    step1="setup"
    step2="configure"
    step3="initialize"
    step4="prepare"
    step5="load"
    step6="validate_input"
    step7="process"
    step8="transform"
    step9="aggregate"
    step10="filter"
    step11="sort"
    step12="format"
    step13="output"
    step14="verify_output"
    step15="cleanup_temp"
    step16="archive"
    step17="notify"
    step18="log_result"
    step19="update_status"
    step20="finalize"
    step21="more_stuff"
    step22="even_more"
    step23="keep_going"
    step24="almost_done"
    step25="final_step"
    [ "$step25" = "final_step" ]
}
