import CoreText
import Darwin
import TextParityFFI
import UIKit
import XCTest

/// Font files under the app sandbox, for a physical device as much as for the simulator (which
/// does not enforce the device's sandbox): the files Core Text reports for the system font, the
/// Chinese fallback and the emoji fallback, opened and mapped with `errno` reported, and the
/// tuned engine's start-up and memory with its fonts mapped (`map-all`) and copied (`copy`).
///
/// On a phone, run each test alone in a fresh process, as the memory numbers assume:
/// `xcodebuild test -scheme TextSpike-Package -destination 'platform=iOS,id=<UDID>'
/// -only-testing:TextSpikeTests/FontAccessTests/testFontFiles` (and `testFontMemoryMapped`,
/// `testFontMemoryCopied`), after building the Rust half for `aarch64-apple-ios`. Results are
/// printed as `SPIKE|` lines and written to SPIKE_OUT.
final class FontAccessTests: XCTestCase {
  var out: URL {
    let path = ProcessInfo.processInfo.environment["SPIKE_OUT"] ?? NSTemporaryDirectory()
    let url = URL(fileURLWithPath: path)
    try? FileManager.default.createDirectory(at: url, withIntermediateDirectories: true)
    return url
  }

  /// Milliseconds since this process started, from the kernel's start time.
  func sinceProcessStart() -> Double {
    var info = kinfo_proc()
    var size = MemoryLayout<kinfo_proc>.stride
    var mib: [Int32] = [CTL_KERN, KERN_PROC, KERN_PROC_PID, getpid()]

    guard sysctl(&mib, 4, &info, &size, nil, 0) == 0 else { return -1 }

    let start = info.kp_proc.p_un.__p_starttime
    let started = Double(start.tv_sec) + Double(start.tv_usec) / 1e6
    return (Date().timeIntervalSince1970 - started) * 1e3
  }

  func fonts() -> [(String, CTFont)] {
    let system = unsafeBitCast(UIFont.systemFont(ofSize: 17), to: CTFont.self)
    var fonts = [("system", system)]

    for (name, text, language) in [("chinese", "你", "zh-Hans"), ("emoji", "😀", "en")] {
      let string = text as CFString
      let font = CTFontCreateForStringWithLanguage(
        system, string, CFRange(location: 0, length: CFStringGetLength(string)), language as CFString)
      fonts.append((name, font))
    }

    return fonts
  }

  func testFontFiles() throws {
    var lines: [String] = []

    for (name, font) in fonts() {
      let postscript = CTFontCopyPostScriptName(font) as String
      let url = CTFontCopyAttribute(font, kCTFontURLAttribute) as? URL
      var line = "FONTFILE \(name) \(postscript) path=\(url?.path ?? "none")"

      if let path = url?.path {
        let started = Date()
        let descriptor = open(path, O_RDONLY)

        if descriptor < 0 {
          line += " open=errno\(errno)"
        } else {
          var info = stat()
          fstat(descriptor, &info)
          let size = Int(info.st_size)
          let mapped = mmap(nil, size, PROT_READ, MAP_PRIVATE, descriptor, 0)

          if mapped == nil || mapped == UnsafeMutableRawPointer(bitPattern: -1) {
            line += " size=\(size) mmap=errno\(errno)"
          } else {
            let head = mapped!.load(as: UInt32.self).bigEndian
            line += String(format: " size=%d mmap=ok head=%08x", size, head)
            munmap(mapped, size)
          }

          close(descriptor)
          line += String(format: " ms=%.3f", Date().timeIntervalSince(started) * 1e3)
        }
      }

      lines.append(line)
    }

    lines.append(String(format: "FONTFILE since process start %.1f ms", sinceProcessStart()))

    for line in lines {
      print("SPIKE| " + line)
    }

    try? (lines.joined(separator: "\n") + "\n").write(
      to: out.appendingPathComponent("font-files.txt"), atomically: true, encoding: .utf8)
  }

  func memory(_ storage: String) {
    let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 390, height: 844))
    window.rootViewController = UIViewController()
    window.makeKeyAndVisible()

    let before = sinceProcessStart()
    let answer = apple_parity_fontmem(storage)!
    let text = String(cString: answer)
    apple_parity_free(answer)

    print(String(format: "SPIKE| FONTMEM %@ probe started %.1f ms after the process", storage, before))
    print("SPIKE| FONTMEM \(text.prefix(4000))")
    try? text.write(to: out.appendingPathComponent("fontmem-xctest-\(storage).json"), atomically: true, encoding: .utf8)
    window.isHidden = true
    XCTAssertFalse(text.hasPrefix("error:"), text)
  }

  func testFontMemoryMapped() throws {
    memory("map-all")
  }

  func testFontMemoryCopied() throws {
    memory("copy")
  }
}
