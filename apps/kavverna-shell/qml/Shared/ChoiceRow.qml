import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

ColumnLayout {
    id: row

    required property var theme
    property alias title: titleLabel.text
    property alias detail: detailLabel.text
    property var choices: []
    property int current: 0
    signal picked(int value)

    Layout.fillWidth: true
    spacing: 6

    Label {
        id: titleLabel
        font.pixelSize: 13
        font.bold: true
        color: row.theme.primaryText
    }

    Label {
        id: detailLabel
        font.pixelSize: 11
        color: row.theme.secondaryText
        wrapMode: Text.WordWrap
        Layout.fillWidth: true
        visible: text.length > 0
    }

    RowLayout {
        Layout.fillWidth: true
        spacing: 4

        Repeater {
            model: row.choices

            delegate: Button {
                required property var modelData
                readonly property bool active: modelData.value === row.current

                Layout.fillWidth: true
                implicitHeight: 26
                text: modelData.label
                font.pixelSize: 11
                leftPadding: 2
                rightPadding: 2
                onClicked: row.picked(modelData.value)

                background: Rectangle {
                    radius: 6
                    color: parent.active ? row.theme.selected : row.theme.control
                }
            }
        }
    }
}
