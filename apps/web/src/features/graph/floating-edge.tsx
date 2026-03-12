import { Position, type EdgeProps, getBezierPath, useInternalNode } from "@xyflow/react";

function getNodeCenter(node: { internals: { positionAbsolute: { x: number; y: number } }; measured: { width?: number; height?: number } }) {
    return {
        x: node.internals.positionAbsolute.x + (node.measured.width ?? 0) / 2,
        y: node.internals.positionAbsolute.y + (node.measured.height ?? 0) / 2
    };
}

/** Snap to the nearest of the 4 cardinal handle points (top, right, bottom, left) */
function getHandlePoint(
    center: { x: number; y: number },
    halfW: number,
    halfH: number,
    otherCenter: { x: number; y: number }
): { x: number; y: number; position: Position } {
    const dx = otherCenter.x - center.x;
    const dy = otherCenter.y - center.y;

    // Pick the side that faces the other node
    if (Math.abs(dx) > Math.abs(dy)) {
        // Horizontal dominant
        if (dx > 0) return { x: center.x + halfW, y: center.y, position: Position.Right };
        return { x: center.x - halfW, y: center.y, position: Position.Left };
    }
    // Vertical dominant
    if (dy > 0) return { x: center.x, y: center.y + halfH, position: Position.Bottom };
    return { x: center.x, y: center.y - halfH, position: Position.Top };
}

function oppositePosition(pos: Position): Position {
    if (pos === Position.Top) return Position.Bottom;
    if (pos === Position.Bottom) return Position.Top;
    if (pos === Position.Left) return Position.Right;
    return Position.Left;
}

export function FloatingEdge({ id, source, target, style, data, label, labelStyle, labelBgStyle }: EdgeProps) {
    const sourceNode = useInternalNode(source);
    const targetNode = useInternalNode(target);

    if (!sourceNode || !targetNode) return null;

    const sourceCenter = getNodeCenter(sourceNode);
    const targetCenter = getNodeCenter(targetNode);

    const sourceW = (sourceNode.measured.width ?? 0) / 2;
    const sourceH = (sourceNode.measured.height ?? 0) / 2;
    const targetW = (targetNode.measured.width ?? 0) / 2;
    const targetH = (targetNode.measured.height ?? 0) / 2;

    const { x: sx, y: sy, position: sourcePosition } = getHandlePoint(sourceCenter, sourceW, sourceH, targetCenter);
    const { x: tx, y: ty, position: targetPosition } = getHandlePoint(targetCenter, targetW, targetH, sourceCenter);

    const si = { x: sx, y: sy };
    const ti = { x: tx, y: ty };

    const curveOffset = (data as { curveOffset?: number })?.curveOffset ?? 0;

    let edgePath: string;
    let lx: number;
    let ly: number;
    let arrivalAngle: number;

    if (curveOffset === 0) {
        const [p, px, py] = getBezierPath({
            sourceX: si.x,
            sourceY: si.y,
            targetX: ti.x,
            targetY: ti.y,
            sourcePosition,
            targetPosition
        });
        edgePath = p;
        lx = px;
        ly = py;
        arrivalAngle = Math.atan2(ti.y - si.y, ti.x - si.x);
    } else {
        const midX = (si.x + ti.x) / 2;
        const midY = (si.y + ti.y) / 2;
        const edgeLen = Math.sqrt((ti.x - si.x) ** 2 + (ti.y - si.y) ** 2) + 1;
        const nx = -(ti.y - si.y) / edgeLen;
        const ny = (ti.x - si.x) / edgeLen;
        const cx = midX + nx * curveOffset;
        const cy = midY + ny * curveOffset;
        edgePath = `M ${si.x} ${si.y} Q ${cx} ${cy} ${ti.x} ${ti.y}`;
        lx = cx;
        ly = cy;
        // For quadratic bezier, the tangent at the end points from control point to end
        arrivalAngle = Math.atan2(ti.y - cy, ti.x - cx);
    }

    const strokeColor = (style as Record<string, string>)?.stroke ?? "#475569";
    const isDirected = (data as { directed?: boolean })?.directed !== false;

    // Compute arrowhead triangle points at the target
    const arrowSize = 8;
    const arrowPoints = isDirected
        ? [
              // Tip at the target intersection
              { x: ti.x, y: ti.y },
              // Two base points spread perpendicular
              {
                  x: ti.x - Math.cos(arrivalAngle - 0.4) * arrowSize,
                  y: ti.y - Math.sin(arrivalAngle - 0.4) * arrowSize
              },
              {
                  x: ti.x - Math.cos(arrivalAngle + 0.4) * arrowSize,
                  y: ti.y - Math.sin(arrivalAngle + 0.4) * arrowSize
              }
          ]
        : null;

    return (
        <>
            {/* Glow layer */}
            <path d={edgePath} style={{ stroke: strokeColor, strokeWidth: 6, strokeOpacity: 0.12, fill: "none" }} />
            {/* Main edge */}
            <path id={id} className="react-flow__edge-path" d={edgePath} style={style} />
            {/* Arrowhead */}
            {arrowPoints && <polygon points={arrowPoints.map(p => `${p.x},${p.y}`).join(" ")} fill={strokeColor} fillOpacity={0.8} />}
            {(() => {
                const bundleCount = (data as { bundleCount?: number })?.bundleCount;
                if (!bundleCount || bundleCount <= 1) return null;
                return (
                    <text
                        x={lx}
                        y={ly}
                        textAnchor="middle"
                        dominantBaseline="central"
                        style={{ fill: strokeColor, fontSize: 9, fontWeight: 600 }}
                    >
                        ×{bundleCount}
                    </text>
                );
            })()}
            {label &&
                (() => {
                    const text = label as string;
                    const charWidth = 6;
                    const padding = 16;
                    const textWidth = text.length * charWidth + padding;
                    return (
                        <>
                            <rect
                                x={lx - textWidth / 2}
                                y={ly - 10}
                                width={textWidth}
                                height={20}
                                rx={6}
                                style={labelBgStyle as React.CSSProperties}
                            />
                            <text x={lx} y={ly} textAnchor="middle" dominantBaseline="central" style={labelStyle as React.CSSProperties}>
                                {text}
                            </text>
                        </>
                    );
                })()}
        </>
    );
}
