#!/bin/bash
# テスト臭いのサンプル集 — shunit2 形式

# 1. Empty Test
test_empty() {
}

# 2. Missing Assertion
test_no_assertion() {
    result=$((2 + 2))
    echo "$result"
}

# 3. Sleepy Test
test_sleepy() {
    sleep 1
    assertEquals 0 $?
}

# 4. Conditional Logic
test_conditional() {
    if [ -f /tmp/testfile ]; then
        assertEquals "exists" "exists"
    fi
}

# 5. Clean test
test_clean() {
    result=$((2 + 2))
    assertEquals 4 "$result"
}

# 6. function keyword syntax
function test_function_keyword {
    result="hello"
    assertEquals "hello" "$result"
}

# 7. Early return
test_early_return() {
    return 0
    assertEquals 1 1
}

. shunit2
