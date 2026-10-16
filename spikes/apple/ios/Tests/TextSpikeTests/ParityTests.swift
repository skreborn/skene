import TextParityFFI
import UIKit
import XCTest

/// The text-parity revisit on the simulator: the Rust suite (spikes/apple/parity) linked as a
/// static library and run in the test process, which has a main thread, a key window and the
/// simulator's fonts. Results go to SPIKE_OUT/parity.
final class ParityTests: XCTestCase {
  var out: URL {
    let path = ProcessInfo.processInfo.environment["SPIKE_OUT"] ?? NSTemporaryDirectory()
    let url = URL(fileURLWithPath: path)
    try? FileManager.default.createDirectory(at: url, withIntermediateDirectories: true)
    return url
  }

  func testParitySuite() throws {
    XCTAssertEqual(text_parity_ffi_linked(), 1)

    let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 390, height: 844))
    window.rootViewController = UIViewController()
    window.makeKeyAndVisible()

    let device = "\(UIDevice.current.model) \(UIDevice.current.systemName) \(UIDevice.current.systemVersion) simulator"
    let parts = ProcessInfo.processInfo.environment["PARITY_PARTS"] ?? ""
    let partsJSON = parts.isEmpty ? "[]" : "[" + parts.split(separator: ",").map { "\"\($0)\"" }.joined(separator: ",") + "]"
    let config = """
      {"out": "\(out.appendingPathComponent("parity").path)", "scales": [2, 3], "device": "\(device)", "parts": \(partsJSON)}
      """
    let answer = apple_parity_run(config)!
    let text = String(cString: answer)
    apple_parity_free(answer)

    for line in text.split(separator: "\n") {
      print("SPIKE| " + line)
    }

    try? (text + "\n").write(to: out.appendingPathComponent("ios-parity.txt"), atomically: true, encoding: .utf8)
    window.isHidden = true
    XCTAssertFalse(text.hasPrefix("error:"), text)
  }

  func testLanguageLineHeight() throws {
    let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 390, height: 844))
    window.rootViewController = UIViewController()
    window.makeKeyAndVisible()

    let answer = apple_parity_language()!
    let text = String(cString: answer)
    apple_parity_free(answer)

    let language = Locale.preferredLanguages.first ?? "unknown"
    print("SPIKE| LANGUAGE \(language) \(text)")
    try? (text + "\n").write(to: out.appendingPathComponent("ios-language-\(language).json"), atomically: true, encoding: .utf8)
    window.isHidden = true
  }
}
