import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import dev.kavverna.shell

ApplicationWindow {
    id: root
    width: 560
    height: 520
    visible: true
    title: "Kavverna"
    color: palette.window

    FeatureList { id: features }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 20
        spacing: 12

        Label {
            text: features.heading
            color: palette.text
            font.pixelSize: 15
            font.bold: true
        }

        ListView {
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            spacing: 6
            model: features.titles

            delegate: Rectangle {
                required property int index
                required property string modelData

                width: ListView.view.width
                height: row.implicitHeight + 16
                radius: 4
                color: palette.alternateBase

                ColumnLayout {
                    id: row
                    anchors.fill: parent
                    anchors.margins: 8
                    spacing: 2

                    Label {
                        text: parent.parent.modelData
                        color: palette.text
                        font.pixelSize: 13
                        font.bold: true
                    }

                    Label {
                        Layout.fillWidth: true
                        text: features.summaries[parent.parent.index]
                        color: palette.placeholderText
                        font.pixelSize: 11
                        wrapMode: Text.WordWrap
                    }
                }
            }
        }
    }
}
