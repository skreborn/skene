import TextSpike
import UIKit
import XCTest

let strings: [(String, String)] = [
  ("latin", "The quick brown fox jumps over the lazy dog, then naps."),
  ("zh-Hans", "敏捷的棕色狐狸跳过了懒狗，然后睡了一觉。"),
  ("ja", "素早い茶色の狐がのろまな犬を飛び越えて、昼寝をした。"),
  ("ko", "빠른 갈색 여우가 게으른 개를 뛰어넘고 낮잠을 잤다."),
  ("thai", "สุนัขจิ้งจอกสีน้ำตาลกระโดดข้ามสุนัขขี้เกียจแล้วนอนหลับ"),
  ("emoji-zwj", "Family 👨‍👩‍👧‍👦 and 🏳️‍🌈 flags 👩🏽‍💻 at work"),
  ("rtl-mixed", "مرحبا بالعالم hello world مرة أخرى 123"),
]

let scale: CGFloat = 3
let reach = 3

enum Report {
  static var lines: [String] = []

  static var directory: URL {
    let path = ProcessInfo.processInfo.environment["SPIKE_OUT"] ?? NSTemporaryDirectory()
    let url = URL(fileURLWithPath: path)
    try? FileManager.default.createDirectory(at: url, withIntermediateDirectories: true)
    return url
  }

  static func add(_ line: String) {
    print("SPIKE| " + line)
    lines.append(line)
  }

  static func flush(_ name: String) {
    let url = directory.appendingPathComponent("ios-\(name).txt")
    try? (lines.joined(separator: "\n") + "\n").write(to: url, atomically: true, encoding: .utf8)
    lines.removeAll()
  }

  static func save(_ bitmap: Bitmap, _ name: String) {
    if let data = bitmap.png() {
      try? data.write(to: directory.appendingPathComponent(name))
    }
  }
}

func f(_ value: CGFloat) -> String {
  String(format: "%.2f", Double(value))
}

func ms(_ body: () -> Void) -> Double {
  let start = CACurrentMediaTime()
  body()
  return (CACurrentMediaTime() - start) * 1000
}

func graphemeOffsets(_ text: String) -> [Int] {
  var offsets = [0]
  var offset = 0

  for character in text {
    offset += character.utf16.count
    offsets.append(offset)
  }

  return offsets
}

/// The offset, in sixths of a point, at which drawing `laid` comes closest to `target`.
func fractionalBest(_ laid: Laid, boxWidth: CGFloat, canvas: CGSize, against target: Bitmap) -> (CGPoint, Diff) {
  var best: (CGPoint, Diff)?

  for j in -6...6 {
    for i in -3...3 {
      let offset = CGPoint(x: CGFloat(i) / 6, y: CGFloat(j) / 6)
      let image = Bitmap.draw(size: canvas, scale: scale) { context in
        laid.draw(in: context, at: CGPoint(x: laid.shift(boxWidth: boxWidth) + offset.x, y: offset.y))
      }
      let diff = image.diff(target)

      if best == nil || diff.mean < best!.1.mean {
        best = (offset, diff)
      }
    }
  }

  return best!
}

func point(_ offset: CGPoint) -> String {
  "(\(f(offset.x)),\(f(offset.y)))pt"
}

final class SpikeTests: XCTestCase {
  /// Criteria (a), (b) and (c) at three Dynamic Type sizes and with Bold Text.
  @MainActor
  func test1Rendering() throws {
    let configurations: [(String, UIContentSizeCategory, UILegibilityWeight)] = [
      ("small", .small, .regular),
      ("large", .large, .regular),
      ("AX-XL", .accessibilityExtraLarge, .regular),
      ("large+bold", .large, .bold),
    ]

    var failures: [String] = []
    let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 1000, height: 1000))
    let controller = UIViewController()
    window.rootViewController = controller
    window.makeKeyAndVisible()

    for (configName, category, weight) in configurations {
      let host = UIView(frame: CGRect(x: 0, y: 0, width: 1000, height: 1000))
      controller.view.addSubview(host)
      host.traitOverrides.preferredContentSizeCategory = category
      host.traitOverrides.legibilityWeight = weight
      host.updateTraitsIfNeeded()
      let font = UIFont.preferredFont(forTextStyle: .body, compatibleWith: host.traitCollection)

      let probe = UILabel()
      probe.adjustsFontForContentSizeCategory = true
      probe.font = UIFont.preferredFont(forTextStyle: .body)
      host.addSubview(probe)
      host.updateTraitsIfNeeded()
      probe.updateTraitsIfNeeded()

      Report.add("")
      Report.add(
        "[\(configName)] traits: category=\(host.traitCollection.preferredContentSizeCategory.rawValue) legibility=\(host.traitCollection.legibilityWeight.rawValue); node font=\(font.fontName) \(f(font.pointSize))pt; UILabel by traits=\(probe.font.fontName) \(f(probe.font.pointSize))pt \(probe.font == font ? "same" : "DIFFERENT"); font.lineHeight=\(f(font.lineHeight)) ascender=\(f(font.ascender)) descender=\(f(font.descender)) leading=\(f(font.leading)); host in a key window"
      )

      for (name, text) in strings {
        let node = TextNodeView(paragraph: Paragraph(text: text, font: font))
        host.addSubview(node)
        let (minimum, maximum) = node.paragraph.intrinsic()
        let measured = node.measure(.maxContent)
        node.frame = CGRect(x: 0, y: 0, width: measured.width, height: measured.height)
        let laid = node.laid(measured.width)

        let label = UILabel()
        label.font = font
        label.textColor = .black
        label.numberOfLines = 1
        label.text = text
        host.addSubview(label)
        let labelFit = label.sizeThatFits(CGSize(width: CGFloat.greatestFiniteMagnitude, height: .greatestFiniteMagnitude))
        label.frame = CGRect(origin: .zero, size: labelFit)

        let field = UITextField()
        field.font = font
        field.textColor = .black
        field.borderStyle = .none
        field.text = text
        host.addSubview(field)
        let fieldFit = field.intrinsicContentSize
        field.frame = CGRect(origin: .zero, size: CGSize(width: max(fieldFit.width, measured.width), height: fieldFit.height))
        field.layoutIfNeeded()

        let canvas = CGSize(
          width: max(measured.width, labelFit.width, field.frame.width).rounded(.up) + 4,
          height: max(measured.height, labelFit.height, fieldFit.height).rounded(.up) + 4)
        let ours = Bitmap.render(node.layer, size: canvas, scale: scale)
        let theirs = Bitmap.render(label.layer, size: canvas, scale: scale)
        let fields = Bitmap.render(field.layer, size: canvas, scale: scale)

        // (a) the node's drawing against its measured box, drawn with room around it.
        let pad: CGFloat = 12
        let padded = CGSize(width: measured.width + 2 * pad, height: measured.height + 2 * pad)
        let free = Bitmap.draw(size: padded, scale: scale) { context in
          laid.draw(in: context, at: CGPoint(x: pad + laid.shift(boxWidth: measured.width), y: pad))
        }
        let labelFree = Bitmap.draw(size: CGSize(width: labelFit.width + 2 * pad, height: labelFit.height + 2 * pad), scale: scale) {
          context in
          context.translateBy(x: pad, y: pad)
          label.layer.render(in: context)
        }

        func overflow(_ ink: CGRect?, _ box: CGSize) -> (String, Bool) {
          guard let ink else {
            return ("no ink", false)
          }

          let sides = [pad - ink.minX, pad - ink.minY, ink.maxX - pad - box.width, ink.maxY - pad - box.height]
          let worst = sides.max() ?? 0
          let text = "ink=(\(f(ink.minX - pad)),\(f(ink.minY - pad)))-(\(f(ink.maxX - pad)),\(f(ink.maxY - pad))) overflow l/t/r/b=\(sides.map(f).joined(separator: "/"))"
          return (text, worst <= 1 / scale + 0.001)
        }

        let (extent, extentOK) = overflow(free.ink(), CGSize(width: measured.width, height: measured.height))
        let (labelExtent, labelExtentOK) = overflow(labelFree.ink(), labelFit)
        let surface = laid.surface.offsetBy(dx: laid.shift(boxWidth: measured.width), dy: 0)
        var surfaceText = "no ink"
        var surfaceOK = false

        if let ink = free.ink() {
          let inked = ink.offsetBy(dx: -pad, dy: -pad)
          surfaceOK = surface.insetBy(dx: -1 / scale - 0.001, dy: -1 / scale - 0.001).contains(inked)
          surfaceText = "renderingSurfaceBounds=(\(f(surface.minX)),\(f(surface.minY)))-(\(f(surface.maxX)),\(f(surface.maxY))) \(surfaceOK ? "contains the ink" : "MISSES ink")"
        }

        // (b) pixel differences.
        let (labelRaw, labelBest) = ours.diff(theirs, reach: reach)
        let (fieldRaw, fieldBest) = ours.diff(fields, reach: reach)

        // (c) min-content against TextKit's own wrap at that width.
        let atMin = node.paragraph.layout(width: minimum)
        let breaks = Set(node.paragraph.breakOpportunities())
        let midSegment = atMin.lineStarts.filter { !breaks.contains($0) }
        let widest = atMin.lineWidths.max() ?? 0
        let minOK = midSegment.isEmpty && atMin.size.width <= minimum + 0.01

        Report.add(
          "\(name): node \(f(measured.width))x\(f(measured.height)) baselines \(f(measured.firstBaseline))/\(f(measured.lastBaseline)) (font ascender \(f(font.ascender))) lines=\(laid.lineCount); UILabel.sizeThatFits \(f(labelFit.width))x\(f(labelFit.height)); UITextField.intrinsic \(f(fieldFit.width))x\(f(fieldFit.height))"
        )
        Report.add("  (a) \(extent) \(extentOK ? "PASS" : "FAIL") | UILabel's own: \(labelExtent) \(labelExtentOK ? "inside" : "OVERFLOWS")")
        Report.add("  (a) \(surfaceText)")
        let (offset, fractional) = fractionalBest(laid, boxWidth: measured.width, canvas: canvas, against: theirs)
        Report.add("  (b) vs UILabel, node drawn at the best sub-pixel offset \(point(offset)): \(fractional)")
        Report.add("  (b) vs UILabel raw \(labelRaw) | best \(labelBest)")
        Report.add("  (b) vs UITextField raw \(fieldRaw) | best \(fieldBest)")
        Report.add(
          "  (c) min=\(f(minimum)) max=\(f(maximum)) at-min: lines=\(atMin.lineCount) used=\(f(atMin.size.width)) widest-line=\(f(widest)) mid-segment-breaks=\(midSegment) \(minOK ? "PASS" : "FAIL")"
        )
        let units = Array(text.utf16)
        let segment = node.paragraph.widestSegment
        let segmentText = String(decoding: units[segment], as: UTF16.self)
        let alone = Paragraph(text: segmentText, font: font).intrinsic().max
        let segmentRects = node.paragraph.layout(width: Paragraph.unbounded).rangeRects(segment.lowerBound, segment.upperBound)
        let rectList = segmentRects.map { "(\(f($0.minX)),\(f($0.minY)),\(f($0.width)),\(f($0.height)))" }.joined(separator: " ")
        let lineTexts = zip(atMin.lineStarts, atMin.lineStarts.dropFirst() + [units.count]).map { start, end in
          "\"" + String(decoding: units[start..<end], as: UTF16.self) + "\""
        }
        let quotedLines = lineTexts.joined(separator: " ")
        Report.add(
          "  (c) widest segment [\(segment.lowerBound),\(segment.upperBound)) \"\(segmentText)\" = \(f(minimum)) in the line, \(f(alone)) laid alone, segment rects \(rectList); \(node.paragraph.breakOpportunities().count) break opportunities; lines at min: \(quotedLines)"
        )

        if !extentOK { failures.append("\(configName)/\(name) (a)") }
        if !surfaceOK { failures.append("\(configName)/\(name) (a surface)") }
        if !minOK { failures.append("\(configName)/\(name) (c)") }

        if configName == "large" {
          Report.save(Bitmap.strip([ours, theirs, fields]), "large-\(name)-node-label-field.png")
        }

        // Wrapped at 160pt against a UILabel of any number of lines.
        let wrapped = node.measure(.definite(160))
        node.frame = CGRect(x: 0, y: 0, width: 160, height: wrapped.height)
        label.numberOfLines = 0
        let labelWrapped = label.sizeThatFits(CGSize(width: 160, height: CGFloat.greatestFiniteMagnitude))
        label.frame = CGRect(x: 0, y: 0, width: 160, height: labelWrapped.height)
        let wrapCanvas = CGSize(width: 164, height: max(wrapped.height, labelWrapped.height).rounded(.up) + 4)
        let oursWrapped = Bitmap.render(node.layer, size: wrapCanvas, scale: scale)
        let theirsWrapped = Bitmap.render(label.layer, size: wrapCanvas, scale: scale)
        let (wrapRaw, wrapBest) = oursWrapped.diff(theirsWrapped, reach: reach)
        Report.add(
          "  wrap@160: node \(f(wrapped.width))x\(f(wrapped.height)) lines=\(node.laid(160).lineCount); UILabel.sizeThatFits \(f(labelWrapped.width))x\(f(labelWrapped.height)); raw \(wrapRaw) | best \(wrapBest)"
        )

        if configName == "large" {
          Report.save(Bitmap.strip([oursWrapped, theirsWrapped]), "large-\(name)-wrap160-node-label.png")
        }

        if name == "rtl-mixed" {
          label.textAlignment = .right
          let rightAligned = Bitmap.render(label.layer, size: wrapCanvas, scale: scale)
          let (rightRaw, rightBest) = oursWrapped.diff(rightAligned, reach: reach)
          Report.add("  wrap@160 against UILabel.textAlignment=.right: raw \(rightRaw) | best \(rightBest)")
          label.textAlignment = .natural
        }

        node.removeFromSuperview()
        label.removeFromSuperview()
        field.removeFromSuperview()
      }

      // Tail truncation at two lines.
      let long = String(repeating: strings[0].1 + " ", count: 3)
      let node = TextNodeView(paragraph: Paragraph(text: long, font: font, maxLines: 2))
      let measured = node.measure(.definite(160))
      node.frame = CGRect(x: 0, y: 0, width: 160, height: measured.height)
      let laid = node.laid(160)
      let label = UILabel()
      label.font = font
      label.textColor = .black
      label.numberOfLines = 2
      label.lineBreakMode = .byTruncatingTail
      label.text = long
      let fit = label.sizeThatFits(CGSize(width: 160, height: CGFloat.greatestFiniteMagnitude))
      label.frame = CGRect(x: 0, y: 0, width: 160, height: fit.height)
      let canvas = CGSize(width: 164, height: max(measured.height, fit.height).rounded(.up) + 4)
      let ours = Bitmap.render(node.layer, size: canvas, scale: scale)
      let theirs = Bitmap.render(label.layer, size: canvas, scale: scale)
      let (raw, best) = ours.diff(theirs, reach: reach)
      Report.add(
        "truncation (2 lines @160): node \(f(measured.width))x\(f(measured.height)) lines=\(laid.lineCount) truncated=\(laid.truncated) linesBottom=\(f(laid.linesBottom)); UILabel.sizeThatFits \(f(fit.width))x\(f(fit.height)); raw \(raw) | best \(best)"
      )

      if configName == "large" {
        Report.save(Bitmap.strip([ours, theirs]), "large-truncation-node-label.png")
      }

      // The same Latin text in a font without a text style.
      if configName == "large" {
        let plain = UIFont.systemFont(ofSize: font.pointSize)
        let plainNode = TextNodeView(paragraph: Paragraph(text: strings[0].1, font: plain))
        let plainMeasured = plainNode.measure(.maxContent)
        let plainLabel = UILabel()
        plainLabel.font = plain
        plainLabel.text = strings[0].1
        let plainFit = plainLabel.sizeThatFits(CGSize(width: CGFloat.greatestFiniteMagnitude, height: .greatestFiniteMagnitude))
        let preferredFit = plainNode.paragraph.layout(width: 160).size
        Report.add(
          "line height, systemFont(ofSize: \(f(font.pointSize))) (no text style): node \(f(plainMeasured.width))x\(f(plainMeasured.height)) vs UILabel \(f(plainFit.width))x\(f(plainFit.height)); lineHeight=\(f(plain.lineHeight)); wrapped@160 node height \(f(preferredFit.height))"
        )
      }

      // An empty text lays out one line.
      let empty = TextNodeView(paragraph: Paragraph(text: "", font: font))
      let emptyMax = empty.measure(.maxContent)
      let emptyLabel = UILabel()
      emptyLabel.font = font
      emptyLabel.text = ""
      Report.add(
        "empty: node \(f(emptyMax.width))x\(f(emptyMax.height)) lines=\(empty.laid(emptyMax.width).lineCount); UILabel.sizeThatFits \(f(emptyLabel.sizeThatFits(CGSize(width: 100, height: 100)).height))pt high"
      )
    }

    window.isHidden = true
    Report.add("")
    Report.add("(a)/(c) failures: \(failures.isEmpty ? "none" : failures.joined(separator: ", "))")
    Report.flush("1-rendering")
  }

  /// Criterion (d): a non-editable text interaction over the node, as a UITextInput.
  @MainActor
  func test2Interaction() throws {
    let font = UIFont.preferredFont(forTextStyle: .body)
    let node = TextNodeView(paragraph: Paragraph(text: strings[0].1, font: font))
    let measured = node.measure(.definite(240))
    node.frame = CGRect(x: 20, y: 100, width: 240, height: measured.height)

    let interaction = UITextInteraction(for: .nonEditable)
    interaction.textInput = node
    node.addInteraction(interaction)
    Report.add(
      "UITextInteraction(.nonEditable): attached=\(node.interactions.contains { $0 is UITextInteraction }) textInput-is-node=\(interaction.textInput === node) gesturesForFailureRequirements=\(interaction.gesturesForFailureRequirements.count) view.gestureRecognizers=\(node.gestureRecognizers?.count ?? 0)"
    )

    let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 390, height: 844))
    let controller = UIViewController()
    window.rootViewController = controller
    controller.view.addSubview(node)
    window.makeKeyAndVisible()
    let can = node.canBecomeFirstResponder
    let became = node.becomeFirstResponder()
    let scene = window.windowScene == nil ? "none" : "present"
    Report.add(
      "window: isKeyWindow=\(window.isKeyWindow) scene=\(scene); canBecomeFirstResponder=\(can) becomeFirstResponder()=\(became) isFirstResponder=\(node.isFirstResponder)"
    )

    node.selectedTextRange = TextSpan(4, 9)
    let rects = node.selectionRects(for: TextSpan(4, 9)).map { $0.rect }
    let rectText = rects.map { rect in "(" + [rect.minX, rect.minY, rect.width, rect.height].map(f).joined(separator: ",") + ")" }
    let selected = node.text(in: TextSpan(4, 9)) ?? "nil"
    Report.add("selection 4..<9 (quick): text=\(selected) rects=\(rectText.joined(separator: " "))")

    for (name, text) in strings {
      node.setParagraph(Paragraph(text: text, font: font))
      let wrapped = node.measure(.definite(160))
      node.frame = CGRect(x: 20, y: 100, width: 160, height: wrapped.height)
      var mismatches: [String] = []
      let offsets = graphemeOffsets(text)

      for offset in offsets {
        let caret = node.caretRect(for: TextPosition(offset))
        let back = (node.closestPosition(to: CGPoint(x: caret.minX, y: caret.midY)) as? TextPosition)?.offset ?? -1

        if back != offset {
          mismatches.append("\(offset)->\(back)")
        }
      }

      Report.add(
        "caret/hit round trip \(name) @160 (\(node.laid(160).lineCount) lines): \(offsets.count - mismatches.count)/\(offsets.count) boundaries return to themselves; mismatches \(mismatches.prefix(8).joined(separator: " "))"
      )
    }

    Report.add(
      "not tested (need a host app and real touches): long-press selection, loupe, selection handles, edit menu, link-tap coexistence, UITextSelectionDisplayInteraction drawing"
    )
    _ = node.resignFirstResponder()
    window.isHidden = true
    Report.flush("2-interaction")
  }

  /// Criterion (e): 1000 mixed labels against UILabel.
  @MainActor
  func test3Performance() throws {
    let font = UIFont.preferredFont(forTextStyle: .body)
    let base = strings.map { $0.1 } + [String(repeating: strings[0].1 + " ", count: 3)]
    let texts = (0..<1000).map { "\(base[$0 % base.count]) #\($0)" }

    for text in texts.prefix(50) {
      _ = TextNodeView(paragraph: Paragraph(text: text, font: font)).measure(.definite(320))
      let label = UILabel()
      label.font = font
      label.numberOfLines = 0
      label.text = text
      _ = label.sizeThatFits(CGSize(width: 320, height: CGFloat.greatestFiniteMagnitude))
    }

    Report.add("1000 mixed labels (8 kinds, a #n suffix each), simulator on the CI runner; ms")

    var coldRatios: [Double] = []
    var widthRatios: [Double] = []

    for round in 1...5 {
      var nodes: [TextNodeView] = []
      var labels: [UILabel] = []

      let nodeCold = ms {
        nodes = texts.map { TextNodeView(paragraph: Paragraph(text: $0, font: font)) }

        for node in nodes {
          let measured = node.measure(.definite(320))
          _ = node.measure(.known(measured.width))
        }
      }

      let labelCold = ms {
        labels = texts.map {
          let label = UILabel()
          label.font = font
          label.numberOfLines = 0
          label.text = $0
          return label
        }

        for label in labels {
          _ = label.sizeThatFits(CGSize(width: 320, height: CGFloat.greatestFiniteMagnitude))
        }
      }

      let coldLayouts = nodes.reduce(0) { $0 + $1.layouts }
      nodes.forEach { $0.resetCounters() }

      let nodeWidth = ms {
        for node in nodes {
          let measured = node.measure(.definite(300))
          _ = node.measure(.known(measured.width))
        }
      }

      let labelWidth = ms {
        for label in labels {
          _ = label.sizeThatFits(CGSize(width: 300, height: CGFloat.greatestFiniteMagnitude))
        }
      }

      let widthLayouts = nodes.reduce(0) { $0 + $1.layouts }
      nodes.forEach { $0.resetCounters() }

      let nodeSame = ms {
        for node in nodes {
          let measured = node.measure(.definite(300))
          _ = node.measure(.known(measured.width))
        }
      }

      let sameLayouts = nodes.reduce(0) { $0 + $1.layouts }
      coldRatios.append(nodeCold / labelCold)
      widthRatios.append(nodeWidth / labelWidth)

      Report.add(
        "round \(round): cold node \(String(format: "%.1f", nodeCold)) vs UILabel \(String(format: "%.1f", labelCold)) (node layouts \(coldLayouts)); width 320->300 node \(String(format: "%.1f", nodeWidth)) vs UILabel \(String(format: "%.1f", labelWidth)) (node layouts \(widthLayouts)); same width again node \(String(format: "%.1f", nodeSame)) (layouts \(sameLayouts))"
      )

      if round == 5 {
        let intrinsic = ms {
          for text in texts {
            _ = Paragraph(text: text, font: font).intrinsic()
          }
        }

        let nodeDraw = ms {
          _ = Bitmap.draw(size: CGSize(width: 320, height: 200), scale: scale) { context in
            for node in nodes {
              node.laid(300).draw(in: context, at: .zero)
            }
          }
        }

        let labelDraw = ms {
          _ = Bitmap.draw(size: CGSize(width: 320, height: 200), scale: scale) { context in
            for label in labels {
              label.drawText(in: CGRect(x: 0, y: 0, width: 300, height: 200))
            }
          }
        }

        Report.add(
          "intrinsic min/max for 1000 fresh paragraphs (incl. build): \(String(format: "%.1f", intrinsic)); draw 1000 at 300pt @\(Int(scale))x into one bitmap: node \(String(format: "%.1f", nodeDraw)) vs UILabel.drawText(in:) \(String(format: "%.1f", labelDraw))"
        )
      }
    }

    let medianCold = coldRatios.sorted()[coldRatios.count / 2]
    let medianWidth = widthRatios.sorted()[widthRatios.count / 2]
    Report.add(String(format: "median over 5 rounds, node/UILabel: cold %.2fx, width change %.2fx", medianCold, medianWidth))

    // One label animating its font size, frame by frame.
    let text = strings[0].1
    let node = TextNodeView(paragraph: Paragraph(text: text, font: font))
    let label = UILabel()
    label.numberOfLines = 0
    label.text = text
    let frames = 60
    var nodeFrames: [Double] = []
    var labelFrames: [Double] = []
    var nodeDrawFrames: [Double] = []
    var labelDrawFrames: [Double] = []

    for frame in 0..<frames {
      let size = 17 + 17 * CGFloat(frame) / CGFloat(frames - 1)
      let sized = font.withSize(size)
      var measured: Measured?

      nodeFrames.append(ms {
        node.setParagraph(Paragraph(text: text, font: sized))
        let fresh = node.measure(.definite(320))
        _ = node.measure(.known(fresh.width))
        measured = fresh
      })

      var fit = CGSize.zero

      labelFrames.append(ms {
        label.font = sized
        fit = label.sizeThatFits(CGSize(width: 320, height: CGFloat.greatestFiniteMagnitude))
      })

      node.frame = CGRect(x: 0, y: 0, width: measured?.width ?? 0, height: measured?.height ?? 0)
      label.frame = CGRect(origin: .zero, size: fit)

      nodeDrawFrames.append(ms {
        _ = Bitmap.render(node.layer, size: CGSize(width: 320, height: 200), scale: scale)
      })

      labelDrawFrames.append(ms {
        _ = Bitmap.render(label.layer, size: CGSize(width: 320, height: 200), scale: scale)
      })
    }

    func summary(_ values: [Double]) -> String {
      let sorted = values.sorted()
      return String(
        format: "mean %.3f median %.3f max %.3f", values.reduce(0, +) / Double(values.count), sorted[sorted.count / 2],
        sorted.last ?? 0)
    }

    Report.add("font size 17->34 over \(frames) frames, per frame layout: node \(summary(nodeFrames)) | UILabel \(summary(labelFrames))")
    Report.add(
      "  per frame bitmap render (includes a 320x200@\(Int(scale))x bitmap alloc and readback): node \(summary(nodeDrawFrames)) | UILabel \(summary(labelDrawFrames))"
    )
    Report.flush("3-performance")
  }
}
