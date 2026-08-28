import QtQuick
import QtQuick.Controls

Label {
    required property var theme

    font.pixelSize: 10
    font.bold: true
    font.letterSpacing: 1.2
    color: theme.secondaryText
}
