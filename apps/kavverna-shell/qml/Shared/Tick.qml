import QtQuick
import QtQuick.Controls

/// A check box in the panel's own colours, for the same reason Toggle exists: the stock one
/// takes the desktop's accent and would be the only blue thing on the page. Like Toggle it
/// shows `on`, what the backend holds, and a click only asks for the other value.
AbstractButton {
    id: tick

    required property var theme
    property bool on: false

    signal switched(bool wanted)

    font.pixelSize: theme.textBody
    padding: 0
    spacing: theme.gapSnug
    hoverEnabled: true
    implicitHeight: Math.max(indicator.implicitHeight, contentItem.implicitHeight)

    Accessible.role: Accessible.CheckBox
    Accessible.checkable: true
    Accessible.checked: tick.on

    onClicked: tick.switched(!tick.on)

    indicator: Rectangle {
        implicitWidth: 18
        implicitHeight: 18
        y: tick.height / 2 - height / 2
        radius: tick.theme.radiusSmall - 2
        color: tick.on ? tick.theme.accent : tick.theme.control
        border.width: 1
        border.color: tick.on ? tick.theme.accent : tick.theme.hairline
        opacity: tick.enabled ? 1 : 0.4

        Label {
            anchors.centerIn: parent
            visible: tick.on
            text: "✓"
            font.pixelSize: tick.theme.textStrong
            font.bold: true
            color: tick.theme.surface
        }
    }

    // Elided rather than left to set its own width. A sound device names itself things like
    // "Ryzen HD Audio Controller Analogue Stereo", and one long label would otherwise widen the
    // column past the panel and cut off every wrapped paragraph beside it.
    contentItem: Label {
        text: tick.text
        font: tick.font
        color: tick.theme.primaryText
        opacity: tick.enabled ? 1 : 0.4
        verticalAlignment: Text.AlignVCenter
        leftPadding: tick.indicator.width + tick.spacing
        elide: Text.ElideRight
    }
}
