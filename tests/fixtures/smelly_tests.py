# テスト臭いのサンプル集 — pytest / unittest 形式
import time
import unittest

import pytest


# 1. Empty Test
def test_empty():
    pass


# 2. Missing Assertion
def test_no_assertion():
    result = 2 + 2
    print(result)


# 3. Sleepy Test
def test_sleepy():
    time.sleep(1)
    assert True


# 4. Conditional Test Logic
def test_conditional():
    if some_condition():
        assert value() == 1
    else:
        assert value() == 2


# 5. Ignored Test (デコレーター)
@pytest.mark.skip(reason="not ready")
def test_skipped():
    assert True


# 6. Redundant Print
def test_with_print():
    result = compute()
    print("result:", result)
    assert result == 4


# 7. Assertion Roulette（メッセージなしの複数アサーション）
def test_assertion_roulette():
    assert first() == 1
    assert second() == 2
    assert third() == 3


# 8. Magic Number Test
def test_magic_number():
    assert calculate_total() == 12345


# 9. Silent Skip（early return）
def test_early_return():
    if not server_available():
        return
    assert ping() == "pong"


# 10. Fragile Test（sleep + 時間依存の成功判定）
def test_fragile_timing():
    start = time.time()
    time.sleep(1)
    run_task()
    assert time.time() - start < 5


# 11. Commented-Out Test
# def test_commented_out():
#     assert 1 == 1


# 12. Clean test（スメルなし）
def test_clean():
    result = 2 + 2
    assert result == 4, "2 + 2 should equal 4"


# unittest 形式
class TestUnittestStyle(unittest.TestCase):
    def test_empty_method(self):
        pass

    def test_unittest_assertions(self):
        self.assertEqual(2 + 2, 4, "addition should work")

    @unittest.skip("legacy")
    def test_skipped_method(self):
        self.assertTrue(True)

    def test_skip_in_body(self):
        self.skipTest("runtime skip")

    def test_sleep_in_loop(self):
        for i in range(3):
            time.sleep(1)
            self.assertLess(i, 3)


# pytest.raises は例外アサーションとして扱う
def test_raises():
    with pytest.raises(ValueError):
        int("not a number")


# async テスト
async def test_async_sleepy():
    import asyncio
    await asyncio.sleep(1)
    assert True
