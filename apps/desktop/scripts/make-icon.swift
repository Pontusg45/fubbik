// Draws the fubbik desktop app icon: a dark rounded square (the macOS icon
// grid's 824/1024 body) with three linked nodes in the theme's accent and
// status colors — the knowledge graph.
//
// Usage: swift make-icon.swift <out.png>   (1024×1024)

import AppKit

let size: CGFloat = 1024
let out = CommandLine.arguments.count > 1 ? CommandLine.arguments[1] : "icon.png"

let rep = NSBitmapImageRep(
    bitmapDataPlanes: nil, pixelsWide: Int(size), pixelsHigh: Int(size),
    bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false,
    colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0)!
NSGraphicsContext.saveGraphicsState()
NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: rep)

func color(_ hex: Int, _ alpha: CGFloat = 1) -> NSColor {
    NSColor(
        srgbRed: CGFloat((hex >> 16) & 0xff) / 255, green: CGFloat((hex >> 8) & 0xff) / 255,
        blue: CGFloat(hex & 0xff) / 255, alpha: alpha)
}

// Body: the standard macOS icon shape, with a soft top-to-bottom gradient.
let inset: CGFloat = 100
let body = NSRect(x: inset, y: inset, width: size - inset * 2, height: size - inset * 2)
let shape = NSBezierPath(roundedRect: body, xRadius: 185, yRadius: 185)
NSGradient(starting: color(0x2c3039), ending: color(0x16181d))!.draw(in: shape, angle: -90)
color(0xffffff, 0.08).setStroke()
shape.lineWidth = 6
shape.stroke()

// Three nodes and their connections.
let nodes: [(CGPoint, CGFloat, Int)] = [
    (CGPoint(x: 360, y: 630), 92, 0x7c8cff),  // accent
    (CGPoint(x: 668, y: 560), 70, 0x4ade80),  // running
    (CGPoint(x: 470, y: 330), 78, 0x60a5fa),  // external
]
let edges = NSBezierPath()
edges.lineWidth = 26
edges.lineCapStyle = .round
for (a, b) in [(0, 1), (1, 2), (2, 0)] {
    edges.move(to: nodes[a].0)
    edges.line(to: nodes[b].0)
}
color(0xe6e8ee, 0.35).setStroke()
edges.stroke()
for (center, radius, hex) in nodes {
    let dot = NSBezierPath(ovalIn: NSRect(x: center.x - radius, y: center.y - radius, width: radius * 2, height: radius * 2))
    color(hex).setFill()
    dot.fill()
    color(0xffffff, 0.25).setStroke()
    dot.lineWidth = 8
    dot.stroke()
}

NSGraphicsContext.restoreGraphicsState()
try! rep.representation(using: .png, properties: [:])!.write(to: URL(fileURLWithPath: out))
