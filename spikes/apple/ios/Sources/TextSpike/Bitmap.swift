#if canImport(UIKit)
import UIKit

/// An RGB image composited over white, top row first.
public struct Bitmap {
  public let width: Int
  public let height: Int
  public let scale: CGFloat
  public let pixels: [UInt8]

  /// Draws into a white bitmap of `size` points at `scale`, in UIKit's coordinates.
  public static func draw(size: CGSize, scale: CGFloat, _ body: (CGContext) -> Void) -> Bitmap {
    let width = max(Int((size.width * scale).rounded(.up)), 1)
    let height = max(Int((size.height * scale).rounded(.up)), 1)
    let context = CGContext(
      data: nil, width: width, height: height, bitsPerComponent: 8, bytesPerRow: width * 4,
      space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
    context.setFillColor(red: 1, green: 1, blue: 1, alpha: 1)
    context.fill(CGRect(x: 0, y: 0, width: width, height: height))
    context.translateBy(x: 0, y: CGFloat(height))
    context.scaleBy(x: scale, y: -scale)
    UIGraphicsPushContext(context)
    body(context)
    UIGraphicsPopContext()
    let data = context.data!.bindMemory(to: UInt8.self, capacity: width * height * 4)
    var pixels = [UInt8](repeating: 255, count: width * height * 3)

    for index in 0..<(width * height) {
      pixels[index * 3] = data[index * 4]
      pixels[index * 3 + 1] = data[index * 4 + 1]
      pixels[index * 3 + 2] = data[index * 4 + 2]
    }

    return Bitmap(width: width, height: height, scale: scale, pixels: pixels)
  }

  /// Renders a layer into a white bitmap of `size` points.
  public static func render(_ layer: CALayer, size: CGSize, scale: CGFloat) -> Bitmap {
    draw(size: size, scale: scale) { context in
      layer.render(in: context)
    }
  }

  func at(_ x: Int, _ y: Int) -> (UInt8, UInt8, UInt8) {
    if x < 0 || y < 0 || x >= width || y >= height {
      return (255, 255, 255)
    }

    let index = (y * width + x) * 3
    return (pixels[index], pixels[index + 1], pixels[index + 2])
  }

  static func blank(_ pixel: (UInt8, UInt8, UInt8)) -> Bool {
    pixel.0 >= 250 && pixel.1 >= 250 && pixel.2 >= 250
  }

  /// The inked pixels' bounds in points, or nil when nothing is inked.
  public func ink() -> CGRect? {
    var minX = Int.max
    var minY = Int.max
    var maxX = -1
    var maxY = -1

    for y in 0..<height {
      for x in 0..<width where !Bitmap.blank(at(x, y)) {
        minX = Swift.min(minX, x)
        minY = Swift.min(minY, y)
        maxX = Swift.max(maxX, x + 1)
        maxY = Swift.max(maxY, y + 1)
      }
    }

    if maxX < 0 {
      return nil
    }

    return CGRect(
      x: CGFloat(minX) / scale, y: CGFloat(minY) / scale, width: CGFloat(maxX - minX) / scale,
      height: CGFloat(maxY - minY) / scale)
  }

  /// The difference with `other` shifted by `(dx, dy)` pixels.
  public func diff(_ other: Bitmap, dx: Int = 0, dy: Int = 0) -> Diff {
    var result = Diff(shift: (dx, dy))
    var total = 0

    for y in 0..<Swift.max(height, other.height) {
      for x in 0..<Swift.max(width, other.width) {
        let a = at(x, y)
        let b = other.at(x - dx, y - dy)

        if Bitmap.blank(a) && Bitmap.blank(b) {
          continue
        }

        result.inked += 1
        let d = Swift.max(
          Int(a.0) > Int(b.0) ? Int(a.0) - Int(b.0) : Int(b.0) - Int(a.0),
          Int(a.1) > Int(b.1) ? Int(a.1) - Int(b.1) : Int(b.1) - Int(a.1),
          Int(a.2) > Int(b.2) ? Int(a.2) - Int(b.2) : Int(b.2) - Int(a.2))
        result.max = Swift.max(result.max, d)
        total += d

        if d > 64 {
          result.over64 += 1
        }
      }
    }

    result.mean = result.inked == 0 ? 0 : Double(total) / Double(result.inked)
    return result
  }

  /// The raw difference, and the least over shifts of up to `reach` pixels.
  public func diff(_ other: Bitmap, reach: Int) -> (raw: Diff, best: Diff) {
    let raw = diff(other)
    var best = raw

    for dy in -reach...reach {
      for dx in -reach...reach {
        let candidate = diff(other, dx: dx, dy: dy)

        if candidate.mean < best.mean {
          best = candidate
        }
      }
    }

    return (raw, best)
  }

  /// A PNG of this bitmap.
  public func png() -> Data? {
    var rgba = [UInt8](repeating: 255, count: width * height * 4)

    for index in 0..<(width * height) {
      rgba[index * 4] = pixels[index * 3]
      rgba[index * 4 + 1] = pixels[index * 3 + 1]
      rgba[index * 4 + 2] = pixels[index * 3 + 2]
    }

    guard
      let provider = CGDataProvider(data: Data(rgba) as CFData),
      let image = CGImage(
        width: width, height: height, bitsPerComponent: 8, bitsPerPixel: 32, bytesPerRow: width * 4,
        space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGBitmapInfo(rawValue: CGImageAlphaInfo.noneSkipLast.rawValue),
        provider: provider, decode: nil, shouldInterpolate: false, intent: .defaultIntent)
    else {
      return nil
    }

    return UIImage(cgImage: image).pngData()
  }

  /// These bitmaps side by side, top-aligned.
  public static func strip(_ images: [Bitmap]) -> Bitmap {
    let width = images.reduce(0) { $0 + $1.width + 4 }
    let height = images.map(\.height).max() ?? 1
    var pixels = [UInt8](repeating: 200, count: width * height * 3)
    var left = 0

    for image in images {
      for y in 0..<image.height {
        for x in 0..<image.width {
          let source = (y * image.width + x) * 3
          let target = (y * width + left + x) * 3
          pixels[target] = image.pixels[source]
          pixels[target + 1] = image.pixels[source + 1]
          pixels[target + 2] = image.pixels[source + 2]
        }
      }

      left += image.width + 4
    }

    return Bitmap(width: width, height: height, scale: images.first?.scale ?? 1, pixels: pixels)
  }
}

/// How far two bitmaps are apart.
public struct Diff: CustomStringConvertible {
  public var max = 0
  public var mean = 0.0
  public var over64 = 0
  public var inked = 0
  public var shift: (Int, Int)

  public init(shift: (Int, Int)) {
    self.shift = shift
  }

  public var share: Double { inked == 0 ? 0 : 100 * Double(over64) / Double(inked) }

  public var description: String {
    String(
      format: "max=%d mean=%.2f over64=%d (%.2f%% of %d inked px) shift=(%d,%d)", max, mean, over64, share, inked,
      shift.0, shift.1)
  }
}
#endif
