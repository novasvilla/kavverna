import QtQuick
import QtQuick.Shapes

/// A line through the last couple of minutes of a reading, drawn behind the bar that shows the
/// reading now. Oldest on the left, so it reads the way a graph is expected to.
///
/// Shapes rather than Canvas: Canvas repaints on the render thread through a texture, and this
/// redraws every time the sampler ticks.
Item {
    id: trace

    required property var theme
    /// Fractions from 0 to 1, oldest first. Fewer than two and nothing is drawn, since a line
    /// through one point is a claim about history nobody has yet.
    property var readings: []
    property color tint: theme.accent

    readonly property int count: readings ? readings.length : 0

    function pointX(index) {
        return trace.count < 2 ? 0 : (index / (trace.count - 1)) * trace.width
    }

    function pointY(index) {
        return trace.height - Math.max(0, Math.min(1, trace.readings[index])) * trace.height
    }

    // Without a ground of its own an idle reading draws a line along the bottom that reads as a
    // divider rather than as a graph at zero.
    Rectangle {
        anchors.fill: parent
        radius: trace.theme.radiusSmall
        color: trace.theme.sunken
    }

    Shape {
        anchors.fill: parent
        visible: trace.count > 1
        preferredRendererType: Shape.CurveRenderer

        // The filled area first, so the line sits on top of its own shading rather than under it.
        ShapePath {
            fillColor: Qt.alpha(trace.tint, 0.16)
            strokeWidth: 0
            strokeColor: "transparent"

            PathSvg {
                path: {
                    if (trace.count < 2) {
                        return ""
                    }
                    let out = "M 0 " + trace.height + " L " + trace.pointX(0) + " " + trace.pointY(0)
                    for (let index = 1; index < trace.count; index += 1) {
                        out += " L " + trace.pointX(index) + " " + trace.pointY(index)
                    }
                    return out + " L " + trace.width + " " + trace.height + " Z"
                }
            }
        }

        ShapePath {
            fillColor: "transparent"
            strokeColor: Qt.alpha(trace.tint, 0.75)
            strokeWidth: 1
            capStyle: ShapePath.RoundCap
            joinStyle: ShapePath.RoundJoin

            PathSvg {
                path: {
                    if (trace.count < 2) {
                        return ""
                    }
                    let out = "M " + trace.pointX(0) + " " + trace.pointY(0)
                    for (let index = 1; index < trace.count; index += 1) {
                        out += " L " + trace.pointX(index) + " " + trace.pointY(index)
                    }
                    return out
                }
            }
        }
    }
}
