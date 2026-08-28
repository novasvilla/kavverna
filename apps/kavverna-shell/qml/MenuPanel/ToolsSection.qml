import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
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
        implicitHeight: body.implicitHeight + section.theme.pad * 2

        ColumnLayout {
            id: body
            anchors.fill: parent
            anchors.margins: section.theme.pad
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

            ChoiceRow {
                theme: section.theme
                title: "No sooner than"
                current: section.hub.jiggle_shortest
                choices: [
                    { label: "1m", value: 1 },
                    { label: "2m", value: 2 },
                    { label: "5m", value: 5 },
                    { label: "10m", value: 10 },
                    { label: "15m", value: 15 }
                ]
                onPicked: (value) => section.hub.choose_jiggle_shortest(value)
            }

            ChoiceRow {
                theme: section.theme
                title: "And no later than"
                detail: "The wait is drawn afresh between the two, so it does not look like a timer."
                current: section.hub.jiggle_longest
                choices: [
                    { label: "2m", value: 2 },
                    { label: "5m", value: 5 },
                    { label: "10m", value: 10 },
                    { label: "15m", value: 15 },
                    { label: "30m", value: 30 }
                ]
                onPicked: (value) => section.hub.choose_jiggle_longest(value)
            }

            ChoiceRow {
                theme: section.theme
                title: "What it does"
                current: section.hub.jiggle_activity
                choices: [
                    { label: "Pointer", value: 0 },
                    { label: "Key", value: 1 },
                    { label: "Both", value: 2 }
                ]
                onPicked: (value) => section.hub.choose_jiggle_activity(value)
            }

            ChoiceRow {
                theme: section.theme
                title: "Which key"
                detail: "For the watchers that count keys rather than pointer movement."
                visible: section.hub.jiggle_activity !== 0
                current: section.hub.jiggle_keystroke
                choices: [
                    { label: "Shift", value: 0 },
                    { label: "Up and down", value: 1 }
                ]
                onPicked: (value) => section.hub.choose_jiggle_keystroke(value)
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
}
