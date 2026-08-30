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

    Layout.fillWidth: true
    spacing: 12

    SectionLabel {
        theme: section.theme
        text: "MOUSE JIGGLE"
    }

    Card {
        theme: section.theme
        spacing: 10

        RowLayout {
            Layout.fillWidth: true
            spacing: 10

            Kirigami.Icon {
                source: "input-mouse"
                implicitWidth: 18
                implicitHeight: 18
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
                enabled: section.hub.jiggle_available
                checked: section.hub.mouse_jiggle
                onToggled: section.hub.choose_mouse_jiggle(checked)
            }
        }

        Label {
            Layout.fillWidth: true
            text: "Moves the pointer somewhere else and can press a key, for the applications that watch for input rather than for a power inhibition."
            font.pixelSize: section.theme.textBody
            color: section.theme.secondaryText
            wrapMode: Text.WordWrap
        }

        // The timing and behaviour rows live in Settings under TOOLS: they are configuration
        // set once, and this page is the quick control.
        Label {
            Layout.fillWidth: true
            text: "Timing and what a nudge does are set in Settings, under Tools."
            font.pixelSize: section.theme.textSmall
            color: section.theme.mutedText
            wrapMode: Text.WordWrap
        }

        PillButton {
            theme: section.theme
            Layout.fillWidth: true
            implicitHeight: 28
            enabled: section.hub.jiggle_available
            text: "Nudge now"
            onClicked: section.hub.nudge_now()
        }
    }
}
