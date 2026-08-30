import QtQuick
import QtQuick.Shapes

/// Energy's mark, drawn rather than named.
///
/// Breeze has no lightning bolt whose silhouette survives being tinted: `flash` is a camera
/// flash, the storm icons lose their bolt behind the cloud, and
/// `preferences-system-power-management` is a disc with the bolt inside it, so flattening it to
/// one colour leaves a plain circle. A battery would be the other option and this machine has
/// none, so promising one would be a lie.
Item {
    id: bolt

    required property color color
    property int size: 18

    implicitWidth: size
    implicitHeight: size

    Shape {
        anchors.fill: parent
        preferredRendererType: Shape.CurveRenderer

        ShapePath {
            fillColor: bolt.color
            strokeWidth: 0

            startX: bolt.width * 0.60
            startY: bolt.height * 0.04
            PathLine { x: bolt.width * 0.22; y: bolt.height * 0.56 }
            PathLine { x: bolt.width * 0.46; y: bolt.height * 0.56 }
            PathLine { x: bolt.width * 0.38; y: bolt.height * 0.96 }
            PathLine { x: bolt.width * 0.78; y: bolt.height * 0.42 }
            PathLine { x: bolt.width * 0.53; y: bolt.height * 0.42 }
            PathLine { x: bolt.width * 0.60; y: bolt.height * 0.04 }
        }
    }
}
