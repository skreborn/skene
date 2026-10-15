#if canImport(UIKit)
import UIKit

/// The width a measure is asked under, as taffy asks it.
public enum Available {
  case minContent
  case maxContent
  case definite(CGFloat)
  case known(CGFloat)
}

/// What a measure answers.
public struct Measured {
  public let width: CGFloat
  public let height: CGFloat
  public let firstBaseline: CGFloat
  public let lastBaseline: CGFloat
}

/// An immutable paragraph: attributed text in one TextKit 2 content storage.
public final class Paragraph {
  public static let unbounded: CGFloat = 1.0e7

  public let text: String
  public let maxLines: Int
  let storage: NSTextContentStorage
  private var cachedIntrinsic: (min: CGFloat, max: CGFloat)?
  /// The UTF-16 range of the widest unbreakable run, once `intrinsic` has run.
  public private(set) var widestSegment = 0..<0

  public init(text: String, font: UIFont, color: UIColor = .black, maxLines: Int = 0) {
    let style = NSMutableParagraphStyle()
    style.lineBreakStrategy = .standard
    style.alignment = .natural
    let attributed = NSAttributedString(
      string: text,
      attributes: [.font: font, .foregroundColor: color, .paragraphStyle: style]
    )
    self.text = text
    self.maxLines = maxLines
    storage = NSTextContentStorage()
    storage.attributedString = attributed
  }

  public var length: Int { (text as NSString).length }

  /// Lays the paragraph out at `width` under its line limit.
  public func layout(width: CGFloat) -> Laid {
    layout(width: width, maxLines: maxLines)
  }

  func layout(width: CGFloat, maxLines: Int) -> Laid {
    let manager = NSTextLayoutManager()
    let container = NSTextContainer(size: CGSize(width: width, height: Paragraph.unbounded))
    container.lineFragmentPadding = 0

    if maxLines > 0 {
      container.maximumNumberOfLines = maxLines
      container.lineBreakMode = .byTruncatingTail
    }

    manager.textContainer = container
    storage.addTextLayoutManager(manager)
    let document = storage.documentRange
    manager.ensureLayout(for: document)
    let usage = manager.usageBoundsForTextContainer
    var lines: [(baseline: CGFloat, start: Int, width: CGFloat, bottom: CGFloat)] = []
    var surface = CGRect.null
    let start = document.location
    let storage = self.storage

    manager.enumerateTextLayoutFragments(from: start, options: [.ensuresLayout]) { fragment in
      let frame = fragment.layoutFragmentFrame
      let offset = storage.offset(from: start, to: fragment.rangeInElement.location)
      surface = surface.union(fragment.renderingSurfaceBounds.offsetBy(dx: frame.minX, dy: frame.minY))

      for line in fragment.textLineFragments {
        let bounds = line.typographicBounds
        lines.append((
          frame.minY + bounds.minY + line.glyphOrigin.y,
          offset + line.characterRange.location,
          bounds.width,
          frame.minY + bounds.maxY
        ))
      }

      return true
    }

    var truncated = false

    if maxLines > 0 && lines.count >= maxLines {
      truncated = layout(width: width, maxLines: 0).lineCount > maxLines
    }

    return Laid(
      paragraph: self,
      manager: manager,
      width: width,
      usage: usage,
      firstBaseline: lines.first?.baseline ?? 0,
      lastBaseline: lines.last?.baseline ?? 0,
      lineStarts: lines.map { $0.start },
      lineWidths: lines.map { $0.width },
      linesBottom: lines.last?.bottom ?? 0,
      surface: surface,
      truncated: truncated
    )
  }

  /// The UTF-16 offsets where a line may break, from the system's line-break tokenizer.
  public func breakOpportunities() -> [Int] {
    let string = text as CFString
    let length = CFStringGetLength(string)
    var breaks: Set<Int> = [0, length]

    guard
      let tokenizer = CFStringTokenizerCreate(
        kCFAllocatorDefault, string, CFRange(location: 0, length: length), kCFStringTokenizerUnitLineBreak, nil)
    else {
      return breaks.sorted()
    }

    while CFStringTokenizerAdvanceToNextToken(tokenizer).rawValue != 0 {
      let range = CFStringTokenizerGetCurrentTokenRange(tokenizer)
      breaks.insert(range.location)
      breaks.insert(range.location + range.length)
    }

    return breaks.sorted()
  }

  /// The min-content and max-content widths.
  ///
  /// The maximum is the width laid out without a limit; the minimum is the widest run between two
  /// break opportunities, its trailing blanks hanging, read from that same layout.
  public func intrinsic() -> (min: CGFloat, max: CGFloat) {
    if let cached = cachedIntrinsic {
      return cached
    }

    let laid = layout(width: Paragraph.unbounded, maxLines: 0)
    let maximum = laid.size.width
    let units = Array(text.utf16)
    let breaks = breakOpportunities()
    var minimum: CGFloat = 0

    for index in 0..<max(breaks.count - 1, 0) {
      let from = breaks[index]
      var to = breaks[index + 1]

      while to > from, let scalar = Unicode.Scalar(units[to - 1]), scalar.properties.isWhitespace {
        to -= 1
      }

      if to > from {
        let width = laid.rangeWidth(from, to)

        if width > minimum {
          minimum = width
          widestSegment = from..<to
        }
      }
    }

    let result = (min: min(minimum, maximum), max: maximum)
    cachedIntrinsic = result
    return result
  }
}

/// One layout of a paragraph at one width, never re-wrapped after it is made.
public final class Laid {
  public let paragraph: Paragraph
  let manager: NSTextLayoutManager
  /// The container width it was laid out at.
  public let width: CGFloat
  /// The used rectangle inside the container.
  public let usage: CGRect
  public let firstBaseline: CGFloat
  public let lastBaseline: CGFloat
  public let lineStarts: [Int]
  public let lineWidths: [CGFloat]
  public let linesBottom: CGFloat
  /// The union of the fragments' rendering surfaces, in container coordinates.
  public let surface: CGRect
  public let truncated: Bool

  init(
    paragraph: Paragraph, manager: NSTextLayoutManager, width: CGFloat, usage: CGRect, firstBaseline: CGFloat,
    lastBaseline: CGFloat, lineStarts: [Int], lineWidths: [CGFloat], linesBottom: CGFloat, surface: CGRect, truncated: Bool
  ) {
    self.paragraph = paragraph
    self.manager = manager
    self.width = width
    self.usage = usage
    self.firstBaseline = firstBaseline
    self.lastBaseline = lastBaseline
    self.lineStarts = lineStarts
    self.lineWidths = lineWidths
    self.linesBottom = linesBottom
    self.surface = surface
    self.truncated = truncated
  }

  deinit {
    paragraph.storage.removeTextLayoutManager(manager)
  }

  public var size: CGSize { usage.size }
  public var lineCount: Int { lineStarts.count }

  /// How far a box of `boxWidth` shifts the container's drawing so the used rectangle sits where
  /// its alignment puts it.
  public func shift(boxWidth: CGFloat) -> CGFloat {
    let slack = width - usage.width
    let factor = slack > 0.01 ? min(max(usage.minX / slack, 0), 1) : 0
    return -usage.minX + factor * max(boxWidth - usage.width, 0)
  }

  func textRange(_ from: Int, _ to: Int) -> NSTextRange? {
    let storage = paragraph.storage
    let start = storage.documentRange.location

    guard let a = storage.location(start, offsetBy: from), let b = storage.location(start, offsetBy: to) else {
      return nil
    }

    return NSTextRange(location: a, end: b)
  }

  /// The rectangles a UTF-16 range covers, in container coordinates.
  public func rangeRects(_ from: Int, _ to: Int, type: NSTextLayoutManager.SegmentType = .standard) -> [CGRect] {
    guard let range = textRange(from, to) else {
      return []
    }

    var rects: [CGRect] = []

    manager.enumerateTextSegments(in: range, type: type, options: [.rangeNotRequired]) { _, frame, _, _ in
      rects.append(frame)
      return true
    }

    return rects
  }

  public func rangeWidth(_ from: Int, _ to: Int) -> CGFloat {
    rangeRects(from, to).reduce(0) { $0 + $1.width }
  }

  /// The caret's rectangle before the UTF-16 offset, in container coordinates.
  public func caret(_ offset: Int) -> CGRect {
    rangeRects(offset, offset, type: .selection).first ?? .null
  }

  /// The UTF-16 offset nearest a point in container coordinates.
  public func hitTest(_ point: CGPoint) -> Int {
    let storage = paragraph.storage
    let start = storage.documentRange.location
    let selections = manager.textSelectionNavigation.textSelections(
      interactingAt: point,
      inContainerAt: start,
      anchors: [],
      modifiers: [],
      selecting: false,
      bounds: CGRect(x: 0, y: 0, width: width, height: Paragraph.unbounded)
    )

    guard let location = selections.first?.textRanges.first?.location else {
      return 0
    }

    return storage.offset(from: start, to: location)
  }

  /// Draws every fragment, the container's origin at `origin`.
  public func draw(in context: CGContext, at origin: CGPoint) {
    manager.enumerateTextLayoutFragments(from: paragraph.storage.documentRange.location, options: [.ensuresLayout]) {
      fragment in
      let frame = fragment.layoutFragmentFrame
      fragment.draw(at: CGPoint(x: frame.minX + origin.x, y: frame.minY + origin.y), in: context)
      return true
    }
  }
}

public final class TextPosition: UITextPosition {
  public let offset: Int

  public init(_ offset: Int) {
    self.offset = offset
    super.init()
  }
}

public final class TextSpan: UITextRange {
  let low: TextPosition
  let high: TextPosition

  public init(_ a: Int, _ b: Int) {
    low = TextPosition(min(a, b))
    high = TextPosition(max(a, b))
    super.init()
  }

  public override var start: UITextPosition { low }
  public override var end: UITextPosition { high }
  public override var isEmpty: Bool { low.offset == high.offset }
}

final class SelectionRect: UITextSelectionRect {
  private let frame: CGRect
  private let first: Bool
  private let last: Bool

  init(_ frame: CGRect, first: Bool, last: Bool) {
    self.frame = frame
    self.first = first
    self.last = last
    super.init()
  }

  override var rect: CGRect { frame }
  override var writingDirection: NSWritingDirection { .natural }
  override var containsStart: Bool { first }
  override var containsEnd: Bool { last }
  override var isVertical: Bool { false }
}

/// A text node: a view drawing its paragraph through TextKit 2, measured by the corrected leaf.
public final class TextNodeView: UIView, UITextInput {
  public private(set) var paragraph: Paragraph
  private var cache: [Laid] = []
  public private(set) var layouts = 0
  public private(set) var measures = 0
  private var selection: TextSpan?
  public weak var inputDelegate: UITextInputDelegate?
  public lazy var tokenizer: UITextInputTokenizer = UITextInputStringTokenizer(textInput: self)

  public init(paragraph: Paragraph) {
    self.paragraph = paragraph
    super.init(frame: .zero)
    isOpaque = false
    backgroundColor = .clear
    contentMode = .redraw
  }

  required init?(coder: NSCoder) {
    fatalError("unused")
  }

  public func setParagraph(_ paragraph: Paragraph) {
    self.paragraph = paragraph
    cache.removeAll()
    setNeedsDisplay()
  }

  public func resetCounters() {
    layouts = 0
    measures = 0
  }

  /// The layout at `width`, from the cache when one serves it.
  ///
  /// A layout at width `w` that used `u` serves any width in `[u, w]`; the cache keeps three.
  public func laid(_ width: CGFloat) -> Laid {
    if let index = cache.firstIndex(where: { $0.width == width || ($0.usage.width <= width + 1e-6 && width <= $0.width) }) {
      let laid = cache.remove(at: index)
      cache.insert(laid, at: 0)
      return laid
    }

    layouts += 1
    let laid = paragraph.layout(width: width)
    cache.insert(laid, at: 0)

    if cache.count > 3 {
      cache.removeLast()
    }

    return laid
  }

  /// The corrected leaf: min-content and max-content are the intrinsic widths, a definite width lays
  /// out there and reports the width used, a known width is kept.
  public func measure(_ available: Available) -> Measured {
    measures += 1
    let request: CGFloat
    let keep: Bool

    switch available {
    case .minContent:
      request = paragraph.intrinsic().min
      keep = true
    case .maxContent:
      request = paragraph.intrinsic().max
      keep = true
    case .definite(let width):
      request = width
      keep = false
    case .known(let width):
      request = width
      keep = true
    }

    let result = self.laid(request)

    return Measured(
      width: keep ? request : min(result.usage.width, request),
      height: result.usage.height,
      firstBaseline: result.firstBaseline,
      lastBaseline: result.lastBaseline
    )
  }

  private var current: Laid { laid(bounds.width) }

  public override func draw(_ rect: CGRect) {
    guard let context = UIGraphicsGetCurrentContext() else {
      return
    }

    let drawn = current
    drawn.draw(in: context, at: CGPoint(x: drawn.shift(boxWidth: bounds.width), y: 0))
  }

  private func toContainer(_ point: CGPoint) -> CGPoint {
    CGPoint(x: point.x - current.shift(boxWidth: bounds.width), y: point.y)
  }

  private func fromContainer(_ rect: CGRect) -> CGRect {
    rect.offsetBy(dx: current.shift(boxWidth: bounds.width), dy: 0)
  }

  private func clamp(_ offset: Int) -> Int {
    min(max(offset, 0), paragraph.length)
  }

  public override var canBecomeFirstResponder: Bool { true }

  // UIKeyInput
  public var hasText: Bool { !paragraph.text.isEmpty }
  public func insertText(_ text: String) {}
  public func deleteBackward() {}

  // UITextInput
  public func text(in range: UITextRange) -> String? {
    guard let range = range as? TextSpan else {
      return nil
    }

    let low = clamp(range.low.offset)
    let high = clamp(range.high.offset)
    return (paragraph.text as NSString).substring(with: NSRange(location: low, length: high - low))
  }

  public func replace(_ range: UITextRange, withText text: String) {}

  public var selectedTextRange: UITextRange? {
    get { selection }
    set {
      inputDelegate?.selectionWillChange(self)
      selection = newValue as? TextSpan
      inputDelegate?.selectionDidChange(self)
    }
  }

  public var markedTextRange: UITextRange? { nil }
  public var markedTextStyle: [NSAttributedString.Key: Any]? {
    get { nil }
    set {}
  }

  public func setMarkedText(_ markedText: String?, selectedRange: NSRange) {}
  public func unmarkText() {}
  public var beginningOfDocument: UITextPosition { TextPosition(0) }
  public var endOfDocument: UITextPosition { TextPosition(paragraph.length) }

  public func textRange(from fromPosition: UITextPosition, to toPosition: UITextPosition) -> UITextRange? {
    guard let from = fromPosition as? TextPosition, let to = toPosition as? TextPosition else {
      return nil
    }

    return TextSpan(from.offset, to.offset)
  }

  public func position(from position: UITextPosition, offset: Int) -> UITextPosition? {
    guard let position = position as? TextPosition else {
      return nil
    }

    let target = position.offset + offset
    return (0...paragraph.length).contains(target) ? TextPosition(target) : nil
  }

  public func position(from position: UITextPosition, in direction: UITextLayoutDirection, offset: Int)
    -> UITextPosition?
  {
    switch direction {
    case .right, .down:
      return self.position(from: position, offset: offset)
    default:
      return self.position(from: position, offset: -offset)
    }
  }

  public func compare(_ position: UITextPosition, to other: UITextPosition) -> ComparisonResult {
    let a = (position as? TextPosition)?.offset ?? 0
    let b = (other as? TextPosition)?.offset ?? 0
    return a < b ? .orderedAscending : (a > b ? .orderedDescending : .orderedSame)
  }

  public func offset(from: UITextPosition, to toPosition: UITextPosition) -> Int {
    ((toPosition as? TextPosition)?.offset ?? 0) - ((from as? TextPosition)?.offset ?? 0)
  }

  public func position(within range: UITextRange, farthestIn direction: UITextLayoutDirection) -> UITextPosition? {
    switch direction {
    case .left, .up:
      return range.start
    default:
      return range.end
    }
  }

  public func characterRange(byExtending position: UITextPosition, in direction: UITextLayoutDirection)
    -> UITextRange?
  {
    guard let position = position as? TextPosition else {
      return nil
    }

    switch direction {
    case .left, .up:
      return TextSpan(clamp(position.offset - 1), position.offset)
    default:
      return TextSpan(position.offset, clamp(position.offset + 1))
    }
  }

  public func baseWritingDirection(for position: UITextPosition, in direction: UITextStorageDirection)
    -> NSWritingDirection
  {
    .natural
  }

  public func setBaseWritingDirection(_ writingDirection: NSWritingDirection, for range: UITextRange) {}

  public func firstRect(for range: UITextRange) -> CGRect {
    guard let range = range as? TextSpan else {
      return .null
    }

    return current.rangeRects(range.low.offset, range.high.offset).first.map(fromContainer) ?? .null
  }

  public func caretRect(for position: UITextPosition) -> CGRect {
    guard let position = position as? TextPosition else {
      return .null
    }

    let caret = fromContainer(current.caret(clamp(position.offset)))
    return CGRect(x: caret.minX, y: caret.minY, width: 2, height: caret.height)
  }

  public func selectionRects(for range: UITextRange) -> [UITextSelectionRect] {
    guard let range = range as? TextSpan else {
      return []
    }

    let rects = current.rangeRects(range.low.offset, range.high.offset, type: .selection).map(fromContainer)

    return rects.enumerated().map { index, rect in
      SelectionRect(rect, first: index == 0, last: index == rects.count - 1)
    }
  }

  public func closestPosition(to point: CGPoint) -> UITextPosition? {
    TextPosition(current.hitTest(toContainer(point)))
  }

  public func closestPosition(to point: CGPoint, within range: UITextRange) -> UITextPosition? {
    guard let range = range as? TextSpan else {
      return nil
    }

    let offset = current.hitTest(toContainer(point))
    return TextPosition(min(max(offset, range.low.offset), range.high.offset))
  }

  public func characterRange(at point: CGPoint) -> UITextRange? {
    let offset = current.hitTest(toContainer(point))
    return TextSpan(offset, clamp(offset + 1))
  }
}
#endif
