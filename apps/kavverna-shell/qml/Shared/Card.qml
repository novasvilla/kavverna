import QtQuick
import QtQuick.Layouts

Rectangle {
    required property var theme

    radius: 10
    color: theme.raised
    Layout.fillWidth: true
}
