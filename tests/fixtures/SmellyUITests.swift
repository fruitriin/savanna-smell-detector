// savanna-smell-detector のフィクスチャ（XCTest / XCUITest）
// XCUIApplication を含むため flavor="xcuitest" と判定され、指摘の文面が E2E 向けに切り替わる。

import XCTest

final class SmellyUITests: XCTestCase {
    // Sleepy Test + Conditional Test Logic + Fragile Test — 自前ポーリング
    func testPollsManually() {
        let app = XCUIApplication()
        app.launch()

        let doneButton = app.buttons["doneButton"]
        var appeared = false
        let deadline = Date().addingTimeInterval(20)
        while Date() < deadline {
            if doneButton.exists {
                appeared = true
                break
            }
            Thread.sleep(forTimeInterval: 0.5)
        }
        XCTAssertTrue(appeared)
    }

    // Assertion Roulette (Strict) — メッセージなしの XCTAssertTrue が並ぶ
    // timeout: 30 はマジックナンバーとして扱わない
    func testWaitsWithoutMessages() {
        let app = XCUIApplication()
        XCTAssertTrue(app.buttons["ok"].waitForExistence(timeout: 30))
        XCTAssertTrue(app.staticTexts["title"].exists)
    }

    // スメルなし — 条件付きの明示スキップは正しい作法（Silent Skip でも Ignored Test でもない）
    func testSkipsProperly() throws {
        guard let id = makeFixture() else {
            throw XCTSkip("深夜帯はフィクスチャを作れない")
        }
        XCTAssertFalse(id.isEmpty, "フィクスチャ ID は空でないこと")
    }

    // スメルなし — completion handler 内のレスポンスパース分岐はテスト本流の分岐ではない
    // （最後の XCTAssertTrue が必ず判定するので黙って通ることはない）
    func testCallbackParsingIsNotConditionalLogic() {
        var found = false
        let expectation = expectation(description: "verify")
        URLSession.shared.dataTask(with: request) { data, _, _ in
            if let data, data.isEmpty == false {
                found = true
            }
            expectation.fulfill()
        }.resume()
        wait(for: [expectation], timeout: 15)
        XCTAssertTrue(found, "レスポンスが取得できること")
    }

    // 失敗を報告してから return するのは Silent Skip ではない
    func testFailsLoudly() {
        let app = XCUIApplication()
        if app.alerts.count > 0 {
            XCTFail("起動直後にアラートが出ている")
            return
        }
        XCTAssertTrue(app.buttons["ok"].exists, "OK ボタンが出ていること")
    }

    // これはテストではないヘルパー（検出されてはいけない）
    private func makeFixture() -> String? {
        "fixture-id"
    }
}

// テストファイルに同居するモック（検出されてはいけない）
final class MockURLProtocol: URLProtocol {
    override func startLoading() {
        print("mock")
    }
}
