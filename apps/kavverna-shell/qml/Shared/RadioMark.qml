import QtQuick

/// The mark beside one choice among several, drawn rather than typed. Breeze has no radio
/// button icon that survives being tinted, and a filled or hollow circle character is whatever
/// the installed font makes of it.
Item {
    id: mark

    required property var theme
    property bool chosen: false

    implicitWidth: 10
    implicitHeight: 10

    Rectangle {
        anchors.fill: parent
        radius: width / 2
        color: "transparent"
        border.width: 1
        border.color: mark.chosen ? mark.theme.accent : mark.theme.secondaryText
    }

    Rectangle {
        anchors.centerIn: parent
        visible: mark.chosen
        width: 4
        height: 4
        radius: 2
        color: mark.theme.accent
    }
}
