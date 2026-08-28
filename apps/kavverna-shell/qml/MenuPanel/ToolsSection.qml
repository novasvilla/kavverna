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
        text: "MOUSE JIGGLE"
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
                    text: "\ud83d\uddb1"
                    font.pixelSize: 16
                    color: section.hub.mouse_jiggle ? section.theme.accent
                                                    : section.theme.secondaryText
                }

                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: 1

                    Label {
                        text: "Nudge the pointer"
                        font.pixelSize: 13
                        font.bold: true
                        color: section.theme.primaryText
                    }

                    Label {
                        Layout.fillWidth: true
                        text: section.hub.jiggle_available
                              ? section.hub.jiggle_status
                              : "Needs ydotool and a running ydotoold"
                        font.pixelSize: 11
                        color: section.hub.mouse_jiggle ? section.theme.accent
                                                        : section.theme.secondaryText
                        elide: Text.ElideRight
                    }
                }

                Switch {
                    enabled: section.hub.jiggle_available
                    checked: section.hub.mouse_jiggle
                    onToggled: section.hub.choose_mouse_jiggle(checked)
                }
            }

            Label {
                Layout.fillWidth: true
                text: "Moves the pointer one step and puts it back, for applications that watch for input rather than power inhibitions."
                font.pixelSize: 11
                color: section.theme.secondaryText
                wrapMode: Text.WordWrap
            }

            ChoiceRow {
                theme: section.theme
                title: "Every"
                current: section.hub.jiggle_minutes
                choices: [
                    { label: "1m", value: 1 },
                    { label: "2m", value: 2 },
                    { label: "5m", value: 5 },
                    { label: "10m", value: 10 },
                    { label: "15m", value: 15 }
                ]
                onPicked: (value) => section.hub.choose_jiggle_minutes(value)
            }

            Button {
                Layout.fillWidth: true
                implicitHeight: 28
                enabled: section.hub.jiggle_available
                text: "Nudge now"
                font.pixelSize: 11
                onClicked: section.hub.nudge_now()

                background: Rectangle {
                    radius: 6
                    color: parent.down ? section.theme.controlDown : section.theme.control
                }
            }
        }
    }
}
