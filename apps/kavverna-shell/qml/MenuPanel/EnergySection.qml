import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../Shared"

ColumnLayout {
    id: section

    required property var theme
    required property var hub

    Layout.fillWidth: true
    spacing: 12

    SectionLabel {
        theme: section.theme
        text: "ENERGY"
    }

    Card {
        theme: section.theme
        implicitHeight: body.implicitHeight + 24

        ColumnLayout {
            id: body
            anchors.fill: parent
            anchors.margins: 12
            spacing: 10

            RowLayout {
                Layout.fillWidth: true
                spacing: 10

                Label {
                    text: "\u26a1"
                    font.pixelSize: 17
                    color: section.theme.warm
                }

                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: 1

                    Label {
                        text: "Keep awake"
                        font.pixelSize: 13
                        font.bold: true
                        color: section.theme.primaryText
                    }

                    Label {
                        Layout.fillWidth: true
                        text: section.hub.awake ? section.hub.awake_summary
                                                : "May suspend when idle"
                        font.pixelSize: 11
                        color: section.theme.secondaryText
                        elide: Text.ElideRight
                    }
                }

                Switch {
                    checked: section.hub.awake
                    onToggled: section.hub.toggle_awake()
                }
            }

            RowLayout {
                Layout.fillWidth: true
                spacing: 4

                Repeater {
                    model: [
                        { label: "15m", minutes: 15 },
                        { label: "30m", minutes: 30 },
                        { label: "1h", minutes: 60 },
                        { label: "2h", minutes: 120 },
                        { label: "4h", minutes: 240 },
                        { label: "8h", minutes: 480 }
                    ]

                    delegate: Button {
                        required property var modelData

                        Layout.fillWidth: true
                        implicitHeight: 26
                        text: modelData.label
                        font.pixelSize: 11
                        leftPadding: 2
                        rightPadding: 2
                        onClicked: section.hub.keep_awake_minutes(modelData.minutes)

                        background: Rectangle {
                            radius: 6
                            color: parent.down ? section.theme.controlDown : section.theme.control
                        }
                    }
                }
            }

            RowLayout {
                Layout.fillWidth: true
                visible: section.hub.timed
                spacing: 6

                Label {
                    text: "Add more"
                    font.pixelSize: 11
                    color: section.theme.secondaryText
                }

                Repeater {
                    model: [
                        { label: "+15m", minutes: 15 },
                        { label: "+30m", minutes: 30 },
                        { label: "+1h", minutes: 60 }
                    ]

                    delegate: Button {
                        required property var modelData

                        Layout.fillWidth: true
                        implicitHeight: 24
                        text: modelData.label
                        font.pixelSize: 11
                        leftPadding: 2
                        rightPadding: 2
                        onClicked: section.hub.extend_minutes(modelData.minutes)

                        background: Rectangle {
                            radius: 6
                            color: parent.down ? section.theme.controlDown : section.theme.control
                        }
                    }
                }
            }

            CheckBox {
                text: "Let displays sleep"
                checked: section.hub.allow_display_sleep
                font.pixelSize: 11
                onToggled: section.hub.choose_display_sleep(checked)
            }
        }
    }
}
