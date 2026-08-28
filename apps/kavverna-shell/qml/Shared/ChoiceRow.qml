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
        visible: text.length > 0
    }

    RowLayout {
        Layout.fillWidth: true
        spacing: 4

        Repeater {
            model: row.choices

            delegate: PillButton {
                required property var modelData

                theme: row.theme
                active: modelData.value === row.current
                Layout.fillWidth: true
                text: modelData.label
                onClicked: row.picked(modelData.value)
            }
        }
    }
}
