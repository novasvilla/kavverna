import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import "../Shared"

ColumnLayout {
    id: section

    required property var theme
    required property var hub
    required property var shows

    /// Asks for the jiggle's own group in Settings.
    signal settingsWanted()

    Layout.fillWidth: true
    spacing: section.theme.pad

    SectionLabel {
        theme: section.theme
        text: "MOUSE JIGGLE"
    }

    Card {
        theme: section.theme
        spacing: section.theme.gap

        RowLayout {
            Layout.fillWidth: true
            spacing: section.theme.gap

            Kirigami.Icon {
                source: "input-mouse"
                implicitWidth: section.theme.iconLarge
                implicitHeight: section.theme.iconLarge
                isMask: true
                color: section.hub.mouse_jiggle ? section.theme.accent
                                                : section.theme.secondaryText
            }

            ColumnLayout {
                Layout.fillWidth: true
                spacing: 1

                Label {
                    text: "Nudge the pointer"
                    font.pixelSize: section.theme.textStrong
                    font.bold: true
                    color: section.theme.primaryText
                }

                Label {
                    Layout.fillWidth: true
                    text: section.hub.jiggle_available
                          ? section.hub.jiggle_status
                          : "Needs ydotool and a running ydotoold"
                    font.pixelSize: section.theme.textBody
                    color: section.hub.mouse_jiggle ? section.theme.accent
                                                    : section.theme.secondaryText
                    elide: Text.ElideRight
                }
            }

            Toggle {
                theme: section.theme
                text: "Nudge the pointer"
                enabled: section.hub.jiggle_available
                on: section.hub.mouse_jiggle
                onSwitched: (wanted) => section.hub.choose_mouse_jiggle(wanted)
            }
        }

        Label {
            Layout.fillWidth: true
            text: "Moves the pointer somewhere else and can press a key, for the applications that watch for input rather than for a power inhibition."
            font.pixelSize: section.theme.textBody
            color: section.theme.secondaryText
            wrapMode: Text.WordWrap
        }

        PillButton {
            theme: section.theme
            Layout.fillWidth: true
            enabled: section.hub.jiggle_available
            text: "Nudge now"
            onClicked: section.hub.nudge_now()
        }

        // The timing and what a nudge does are set once and left, so they live in Settings
        // alone; two copies of the same rows were two places to keep in step.
        PillButton {
            theme: section.theme
            Layout.fillWidth: true
            text: "Timing and keys…"
            onClicked: section.settingsWanted()
        }
    }
}
