import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../Shared"

ColumnLayout {
    id: section

    required property var theme
    required property var hub
    required property var shows

    Layout.fillWidth: true
    spacing: 12

    SectionLabel {
        theme: section.theme
        text: "ENERGY"
    }

    Card {
        theme: section.theme
        spacing: 10

        RowLayout {
            Layout.fillWidth: true
            spacing: 10

            Bolt {
                color: section.hub.awake ? section.theme.accent : section.theme.secondaryText
            }

            ColumnLayout {
                Layout.fillWidth: true
                spacing: 1

                Label {
                    text: "Keep awake"
                    font.pixelSize: section.theme.textStrong
                    font.bold: true
                    color: section.theme.primaryText
                }

                Label {
                    Layout.fillWidth: true
                    text: section.hub.awake_summary
                    font.pixelSize: section.theme.textBody
                    color: section.theme.secondaryText
                    elide: Text.ElideRight
                }
            }

            Toggle {
                theme: section.theme
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

                delegate: PillButton {
                    required property var modelData

                    theme: section.theme
                    Layout.fillWidth: true
                    text: modelData.label
                    onClicked: section.hub.keep_awake_minutes(modelData.minutes)
                }
            }
        }

        RowLayout {
            Layout.fillWidth: true
            visible: section.hub.timed
            spacing: 6

            Label {
                text: "Add more"
                font.pixelSize: section.theme.textBody
                color: section.theme.secondaryText
            }

            Repeater {
                model: [
                    { label: "+15m", minutes: 15 },
                    { label: "+30m", minutes: 30 },
                    { label: "+1h", minutes: 60 }
                ]

                delegate: PillButton {
                    required property var modelData

                    theme: section.theme
                    Layout.fillWidth: true
                    implicitHeight: 24
                    text: modelData.label
                    onClicked: section.hub.extend_minutes(modelData.minutes)
                }
            }
        }

        Tick {
            theme: section.theme
            text: "Let displays sleep"
            checked: section.hub.allow_display_sleep
            onToggled: section.hub.choose_display_sleep(checked)
        }
    }
}
