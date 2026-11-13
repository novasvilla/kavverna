import QtQuick
import QtQuick.Controls
import org.kde.kirigami as Kirigami

/// An icon button for something that cannot be undone. The first press only arms it: the icon
/// gives way to the action spelled out in ember, and a second press within a few seconds is
/// what acts. Left alone, it disarms. The confirmation stays in the button rather than in a
/// dialog, since a popup over a layer surface is not reliably shown.
Pressable {
    id: button

    required property string source
    /// What the second press does, written on the button once it is armed.
    required property string confirmText
    property int size: theme.iconSmall
    property bool tall: false
    property bool armed: false

    signal confirmed()

    readonly property int square: tall ? theme.buttonHeight : theme.buttonHeightCompact

    implicitHeight: square
    implicitWidth: armed ? warning.implicitWidth + 2 * theme.gap : square
    fill: armed ? theme.emberGlow
        : down ? theme.controlDown : "transparent"

    Accessible.name: armed ? confirmText : text
    ToolTip.visible: hovered && !armed && text.length > 0
    ToolTip.text: text

    onClicked: {
        if (armed) {
            armed = false
            confirmed()
        } else {
            armed = true
            settle.restart()
        }
    }
    onEnabledChanged: if (!enabled) armed = false

    Timer {
        id: settle
        interval: 4000
        onTriggered: button.armed = false
    }

    contentItem: Item {
        Kirigami.Icon {
            anchors.centerIn: parent
            visible: !button.armed
            source: button.source
            implicitWidth: button.size
            implicitHeight: button.size
            color: button.theme.secondaryText
            isMask: true
            opacity: button.enabled ? (button.hovered ? 1 : 0.75) : 0.35
        }

        Label {
            id: warning
            anchors.centerIn: parent
            visible: button.armed
            text: button.confirmText
            font.pixelSize: button.theme.textSmall
            font.bold: true
            color: button.theme.ember
        }
    }
}
