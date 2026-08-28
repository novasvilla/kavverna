import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../Shared"

ColumnLayout {
    id: section

    required property var theme
    required property var vitals

    Layout.fillWidth: true
    spacing: 12

    component Meter: ColumnLayout {
        required property var theme
        property alias label: meterLabel.text
        property alias value: meterValue.text
        property real fraction: 0
        property color tint: theme.accent

        Layout.fillWidth: true
        spacing: 3

        RowLayout {
            Layout.fillWidth: true

            Label {
                id: meterLabel
                Layout.fillWidth: true
                font.pixelSize: 11
                color: theme.secondaryText
            }

            Label {
                id: meterValue
                font.pixelSize: 11
                font.bold: true
                color: theme.primaryText
            }
        }

        Rectangle {
            Layout.fillWidth: true
            implicitHeight: 5
            radius: 3
            color: theme.control

            Rectangle {
                width: parent.width * Math.max(0, Math.min(1, fraction))
                height: parent.height
                radius: 3
                color: tint
            }
        }
    }

    SectionLabel {
        theme: section.theme
        text: "PROCESSOR AND MEMORY"
    }

    Card {
        theme: section.theme
        implicitHeight: system.implicitHeight + 24

        ColumnLayout {
            id: system
            anchors.fill: parent
            anchors.margins: 12
            spacing: 10

            Meter {
                theme: section.theme
                label: "CPU  ·  " + section.vitals.cpu_temperature_text
                value: section.vitals.cpu_load_text
                fraction: section.vitals.cpu_load
                tint: section.vitals.cpu_load > 0.9 ? section.theme.ember
                    : section.vitals.cpu_load > 0.7 ? section.theme.warm
                                                    : section.theme.accent
            }

            RowLayout {
                Layout.fillWidth: true
                spacing: 2
                visible: section.vitals.core_loads.length > 0

                Repeater {
                    model: section.vitals.core_loads.length

                    delegate: Rectangle {
                        required property int index

                        Layout.fillWidth: true
                        implicitHeight: 16
                        radius: 2
                        color: section.theme.control

                        Rectangle {
                            anchors.bottom: parent.bottom
                            width: parent.width
                            height: parent.height * section.vitals.core_loads[parent.index]
                            radius: 2
                            color: section.theme.accent
                        }
                    }
                }
            }

            Meter {
                theme: section.theme
                label: "Memory"
                value: section.vitals.memory_text
                fraction: section.vitals.memory_used
            }

            Meter {
                theme: section.theme
                label: "Held by applications"
                value: section.vitals.memory_apps_text
                fraction: section.vitals.memory_apps
                tint: section.theme.secondaryText
            }

            RowLayout {
                Layout.fillWidth: true

                Label {
                    Layout.fillWidth: true
                    text: "Pressure"
                    font.pixelSize: 11
                    color: section.theme.secondaryText
                }

                Label {
                    text: section.vitals.pressure_text
                    font.pixelSize: 11
                    color: section.theme.primaryText
                }
            }

            RowLayout {
                Layout.fillWidth: true

                Label {
                    Layout.fillWidth: true
                    text: "Compressed swap"
                    font.pixelSize: 11
                    color: section.theme.secondaryText
                }

                Label {
                    text: section.vitals.swap_text
                    font.pixelSize: 11
                    color: section.theme.primaryText
                }
            }
        }
    }

    SectionLabel {
        theme: section.theme
        text: "GRAPHICS"
    }

    Card {
        theme: section.theme
        implicitHeight: graphics.implicitHeight + 24

        ColumnLayout {
            id: graphics
            anchors.fill: parent
            anchors.margins: 12
            spacing: 10

            RowLayout {
                Layout.fillWidth: true
                spacing: 4
                visible: section.vitals.card_names.length > 1

                Repeater {
                    model: section.vitals.card_names

                    delegate: Button {
                        required property int index
                        required property string modelData

                        Layout.fillWidth: true
                        implicitHeight: 24
                        text: modelData
                        font.pixelSize: 10
                        leftPadding: 4
                        rightPadding: 4
                        onClicked: section.vitals.choose_card(index)

                        background: Rectangle {
                            radius: 6
                            color: section.vitals.chosen_card === parent.index
                                   ? section.theme.selected : section.theme.control
                        }
                    }
                }
            }

            Meter {
                theme: section.theme
                label: "GPU  ·  " + section.vitals.gpu_temperature_text
                       + "  ·  " + section.vitals.gpu_power_text
                value: section.vitals.gpu_usage_text
                fraction: section.vitals.gpu_usage
            }

            Meter {
                theme: section.theme
                label: "VRAM"
                value: section.vitals.vram_text
                fraction: section.vitals.vram_used
                tint: section.vitals.vram_used > 0.9 ? section.theme.ember : section.theme.warm
            }
        }
    }
}
