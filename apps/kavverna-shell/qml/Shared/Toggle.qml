import QtQuick
import QtQuick.Controls

/// A Switch wearing the panel's own colours. The stock one follows the desktop's accent, which
/// left every switch Breeze blue in a panel that is otherwise lit by torchlight.
Switch {
    id: toggle

    required property var theme

    implicitWidth: 40
    implicitHeight: 22
    padding: 0

    indicator: Rectangle {
        anchors.fill: parent
        radius: height / 2
        color: toggle.checked ? toggle.theme.accent : toggle.theme.control
        border.width: 1
        border.color: toggle.checked ? toggle.theme.accent : toggle.theme.hairline
        opacity: toggle.enabled ? 1 : 0.4

        Rectangle {
            width: parent.height - 6
            height: width
            radius: width / 2
            y: 3
            x: toggle.checked ? parent.width - width - 3 : 3
            // The knob has to read against whichever half of the track it is sitting on.
            color: toggle.checked ? toggle.theme.surface : toggle.theme.secondaryText

            Behavior on x {
                NumberAnimation { duration: 110; easing.type: Easing.OutCubic }
            }
        }
    }
}
