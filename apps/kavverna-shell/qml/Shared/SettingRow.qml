import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

RowLayout {
    id: row

    required property var theme
    property alias title: titleLabel.text
    property alias detail: detailLabel.text
    property bool on: false
    signal toggled(bool value)

    Layout.fillWidth: true
    spacing: 10

    ColumnLayout {
        Layout.fillWidth: true
        spacing: 1

        Label {
            id: titleLabel
            font.pixelSize: row.theme.textStrong
            font.bold: true
            color: row.theme.primaryText
        }

        Label {
            id: detailLabel
            font.pixelSize: row.theme.textBody
            color: row.theme.secondaryText
            wrapMode: Text.WordWrap
            Layout.fillWidth: true
        }
    }

    Toggle {
        theme: row.theme
        checked: row.on
        onToggled: row.toggled(checked)
    }
}
