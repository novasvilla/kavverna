import QtQuick
import QtQuick.Controls

/// The one small button in the panel, whether it fires something or stands for a choice already
/// made. `active` is the second of those: a choice that is current stays lit while a button that
/// only ever acts never is.
Button {
    id: pill

    required property var theme
    property bool active: false

    implicitHeight: 26
    font.pixelSize: theme.textBody
    leftPadding: pill.theme.gapTight / 2
    rightPadding: pill.theme.gapTight / 2

    contentItem: Label {
        text: pill.text
        font: pill.font
        color: pill.theme.primaryText
        opacity: pill.enabled ? 1 : 0.4
        horizontalAlignment: Text.AlignHCenter
        verticalAlignment: Text.AlignVCenter
        elide: Text.ElideRight
    }

    background: Rectangle {
        radius: pill.theme.radiusSmall
        color: pill.active ? pill.theme.selected
             : pill.down ? pill.theme.controlDown
                         : pill.theme.control
    }
}
