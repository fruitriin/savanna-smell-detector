// savanna-smell-detector のフィクスチャ（Swift Testing）
// わざと臭うテストを並べてある。ここを直してはいけない。

import Foundation
import Testing
@testable import Sample

// Empty Test
@Test func emptyTest() {
}

// Missing Assertion — 実行しているだけ
@Test("リマインダーを作れる")
func missingAssertion() {
    let vm = ReminderEditViewModel()
    vm.name = "散歩"
    vm.makeCreateRequest()
}

// Sleepy Test + Fragile Test
@Test func sleepyTest() async throws {
    try await Task.sleep(nanoseconds: 500_000_000)
    let deadline = Date().addingTimeInterval(30)
    #expect(Date() < deadline)
}

// Silent Skip — 23時台に実行すると黙って通る
@Test func silentSkip() {
    let calendar = Calendar.current
    let oneHourLater = calendar.date(byAdding: .hour, value: 1, to: Date())!
    guard calendar.isDate(oneHourLater, inSameDayAs: Date()) else { return }
    #expect(calendar.isDate(oneHourLater, inSameDayAs: Date()))
}

// Conditional Test Logic
@Test func conditionalLogic() {
    if isWeekend {
        #expect(schedule.isEmpty)
    } else {
        #expect(schedule.isEmpty == false)
    }
}

// Redundant Print
@Test func redundantPrint() {
    let result = calculate()
    print("result: \(result)")
    dump(result)
    #expect(result == 3)
}

// Assertion Roulette — メッセージなしの #expect が並ぶ
@Test func assertionRoulette() {
    #expect(req.isOneShot == false)
    #expect(req.notificationTime == "09:30")
    #expect(req.repeatDays == [1, 2, 3])
}

// Magic Number Test
@Test func magicNumber() {
    #expect(response.statusCode == 201)
    #expect(payload.count == 4096)
}

// Ignored Test
@Test(.disabled("Plan 0042 で直す"))
func ignoredTest() {
    #expect(true)
}

// Suite ごと無効化されている
@Suite(.disabled("バックエンド待ち"))
struct DisabledSuite {
    @Test func inner() {
        #expect(true)
    }
}

// Commented-Out Test
// @Test func commentedOutTest() {
//     #expect(true)
// }

// これはテストではないヘルパー（検出されてはいけない）
private func makeViewModel() -> ReminderEditViewModel {
    ReminderEditViewModel()
}
