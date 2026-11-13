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
    spacing: section.theme.pad

    SectionLabel {
        theme: section.theme
        text: "ENERGY"
    }

    Card {
        theme: section.theme
        spacing: section.theme.gap

        RowLayout {
            Layout.fillWidth: true
            spacing: section.theme.gap

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
                text: "Keep awake"
                on: section.hub.awake
                onSwitched: section.hub.toggle_awake()
            }
        }

        RowLayout {
            Layout.fillWidth: true
            spacing: section.theme.gapTight

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
                    active: section.hub.awake && section.hub.hold_preset === modelData.minutes
                    onClicked: section.hub.keep_awake_minutes(modelData.minutes)
                }
            }
        }

        RowLayout {
            Layout.fillWidth: true
            visible: section.hub.timed
            spacing: section.theme.gapSnug

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
                    compact: true
                    text: modelData.label
                    onClicked: section.hub.extend_minutes(modelData.minutes)
                }
            }
        }

        // A switch, as in Settings: a setting that is on or off is a Toggle everywhere, and a
        // Tick is for picking members of a set, such as which outputs the shortcut visits.
        SettingRow {
            theme: section.theme
            title: "Let displays sleep"
            detail: "Screens still turn off while the system stays up."
            on: section.hub.allow_display_sleep
            onToggled: (value) => section.hub.choose_display_sleep(value)
        }
    }
}
