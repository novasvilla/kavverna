import QtQuick
import QtQuick.Controls
import org.kde.kirigami as Kirigami

/// A bare icon that answers a tap, drawn from the desktop's own theme rather than from a
/// character. A glyph is whatever font happens to be installed, and an emoji is somebody else's
/// colours in the middle of ours.
AbstractButton {
    id: button

    required property var theme
    required property string source
    property int size: 16

    implicitWidth: size + theme.gapSnug
    implicitHeight: size + theme.gapSnug
    hoverEnabled: true

    contentItem: Kirigami.Icon {
        source: button.source
        implicitWidth: button.size
        implicitHeight: button.size
        color: button.theme.secondaryText
        isMask: true
        opacity: button.enabled ? (button.hovered ? 1 : 0.75) : 0.35
    }

    background: Rectangle {
        radius: button.theme.radiusSmall
        color: button.down ? button.theme.controlDown : "transparent"
    }
}
