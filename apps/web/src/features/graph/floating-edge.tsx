import { type EdgeProps, getBezierPath, useInternalNode } from "@xyflow/react";

function getNodeCenter(node: { internals: { positionAbsolute: { x: number; y: number } }; measured: { width?: number; height?: number } }) {
    return {
        x: node.internals.positionAbsolute.x + (node.measured.width ?? 0) / 2,
        y: node.internals.positionAbsolute.y + (node.measured.height ?? 0) / 2
    };
}

function getRectIntersection(center: { x: number; y: number }, angle: number, halfW: number, halfH: number) {
    const cos = Math.cos(angle);
    const sin = Math.sin(angle);
    const scaleX = halfW > 0 ? Math.abs(halfW / cos) : Infinity;
    const scaleY = halfH > 0 ? Math.abs(halfH / sin) : Infinity;
    const scale = Math.min(scaleX, scaleY);
    return { x: center.x + cos * scale, y: center.y + sin * scale };
}

export function FloatingEdge({ id, source, target, style, data, label, labelStyle, labelBgStyle }: EdgeProps) {
    const sourceNode = useInternalNode(source);
    const targetNode = useInternalNode(target);

    if (!sourceNode || !targetNode) return null;

    const sourceCenter = getNodeCenter(sourceNode);
    const targetCenter = getNodeCenter(targetNode);
    const dx = targetCenter.x - sourceCenter.x;
    const dy = targetCenter.y - sourceCenter.y;
    const angle = Math.atan2(dy, dx);

    const sourceW = (sourceNode.measured.width ?? 0) / 2;
    const sourceH = (sourceNode.measured.height ?? 0) / 2;
    const targetW = (targetNode.measured.width ?? 0) / 2;
    const targetH = (targetNode.measured.height ?? 0) / 2;

    const si = getRectIntersection(sourceCenter, angle, sourceW, sourceH);
    const ti = getRectIntersection(targetCenter, angle + Math.PI, targetW, targetH);

    const curveOffset = (data as { curveOffset?: number })?.curveOffset ?? 0;

    let edgePath: string;
    let lx: number;
    let ly: number;
    // Direction angle at the target end (for arrowhead)
    let arrivalAngle: number;

    if (curveOffset === 0) {
        const [p, px, py] = getBezierPath({ sourceX: si.x, sourceY: si.y, targetX: ti.x, targetY: ti.y });
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
            {arrowPoints && (
                <polygon
                    points={arrowPoints.map(p => `${p.x},${p.y}`).join(" ")}
                    fill={strokeColor}
                    fillOpacity={0.8}
                />
            )}
            {label && (
                <>
                    <rect
                        x={lx - 30}
                        y={ly - 10}
                        width={60}
                        height={20}
                        rx={6}
                        style={labelBgStyle as React.CSSProperties}
                    />
                    <text
                        x={lx}
                        y={ly}
                        textAnchor="middle"
                        dominantBaseline="central"
                        style={labelStyle as React.CSSProperties}
                    >
                        {label as string}
                    </text>
                </>
            )}
        </>
    );
}
