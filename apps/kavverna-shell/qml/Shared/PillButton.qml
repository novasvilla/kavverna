import QtQuick
import QtQuick.Controls

/// The one small button in the panel, whether it fires something or stands for a choice already
/// made. `active` is the second of those: a choice that is current stays lit while a button that
/// only ever acts never is.
Pressable {
    id: pill

    property bool active: false

    // Room to breathe when the button sizes itself; a row of them squeezed by a layout still
    // keeps only the thin padding below, so short labels fit before any of them elide.
    implicitWidth: contentItem.implicitWidth + 2 * theme.gap
    implicitHeight: 26
    font.pixelSize: theme.textBody
    leftPadding: pill.theme.gapTight / 2
    rightPadding: pill.theme.gapTight / 2
    fill: pill.active ? pill.theme.selected
        : pill.down ? pill.theme.controlDown
                    : pill.theme.control

    Accessible.checkable: pill.active
    Accessible.checked: pill.active

    contentItem: Label {
        text: pill.text
        font: pill.font
        color: pill.theme.primaryText
        opacity: pill.enabled ? 1 : 0.4
        horizontalAlignment: Text.AlignHCenter
        verticalAlignment: Text.AlignVCenter
        elide: Text.ElideRight
    }
}
