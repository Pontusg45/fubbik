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

export function FloatingEdge({ id, source, target, style, markerEnd, label, labelStyle, labelBgStyle }: EdgeProps) {
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

    const [path, labelX, labelY] = getBezierPath({
        sourceX: si.x,
        sourceY: si.y,
        targetX: ti.x,
        targetY: ti.y
    });

    return (
        <>
            <path id={id} className="react-flow__edge-path" d={path} style={style} markerEnd={markerEnd as string} />
            {label && (
                <>
                    <rect
                        x={labelX - 30}
                        y={labelY - 10}
                        width={60}
                        height={20}
                        rx={4}
                        style={labelBgStyle as React.CSSProperties}
                    />
                    <text
                        x={labelX}
                        y={labelY}
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
