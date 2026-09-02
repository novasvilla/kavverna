import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../Shared"

ColumnLayout {
    id: section

    required property var theme
    required property var vitals
    required property var shows

    Layout.fillWidth: true
    spacing: 12

    component Meter: ColumnLayout {
        id: meter

        required property var theme
        property alias label: meterLabel.text
        property alias value: meterValue.text
        property real fraction: 0
        property color tint: theme.accent
        /// The last couple of minutes of this reading. Left out for a meter whose past says
        /// nothing, such as how much of memory applications are holding.
        property var past: []

        Layout.fillWidth: true
        spacing: 3

        RowLayout {
            Layout.fillWidth: true

            Label {
                id: meterLabel
                Layout.fillWidth: true
                font.pixelSize: section.theme.textBody
                color: theme.secondaryText
            }

            Label {
                id: meterValue
                font.pixelSize: section.theme.textBody
                font.bold: true
                color: theme.primaryText
            }
        }

        Trace {
            theme: meter.theme
            Layout.fillWidth: true
            implicitHeight: 26
            visible: meter.past && meter.past.length > 1
            readings: meter.past
            tint: meter.tint
        }

        Rectangle {
            Layout.fillWidth: true
            implicitHeight: 5
            radius: 3
            color: theme.control

            Rectangle {
                width: parent.width * Math.max(0, Math.min(1, meter.fraction))
                height: parent.height
                radius: 3
                color: meter.tint
            }
        }
    }

    SectionLabel {
        theme: section.theme
        text: "PROCESSOR AND MEMORY"
    }

    Card {
        theme: section.theme
        spacing: 10

        ColumnLayout {
            Layout.fillWidth: true
            visible: section.vitals.processor_name.length > 0
            spacing: 1

            Label {
                Layout.fillWidth: true
                text: section.vitals.processor_name
                font.pixelSize: section.theme.textBody
                font.bold: true
                color: section.theme.primaryText
                elide: Text.ElideRight
            }

            Label {
                Layout.fillWidth: true
                text: section.vitals.processor_detail
                font.pixelSize: section.theme.textFine
                color: section.theme.mutedText
                elide: Text.ElideRight
            }
        }

        Meter {
            theme: section.theme
            label: "CPU  ·  " + section.vitals.cpu_temperature_text
            value: section.vitals.cpu_load_text
            fraction: section.vitals.cpu_load
            past: section.vitals.cpu_history
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
            past: section.vitals.memory_history
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
                font.pixelSize: section.theme.textBody
                color: section.theme.secondaryText
            }

            Label {
                text: section.vitals.pressure_text
                font.pixelSize: section.theme.textBody
                color: section.theme.primaryText
            }
        }

        RowLayout {
            Layout.fillWidth: true

            Label {
                Layout.fillWidth: true
                text: "Compressed swap"
                font.pixelSize: section.theme.textBody
                color: section.theme.secondaryText
            }

            Label {
                text: section.vitals.swap_text
                font.pixelSize: section.theme.textBody
                color: section.theme.primaryText
            }
        }
    }

    SectionLabel {
        theme: section.theme
        text: "GRAPHICS"
    }

    Card {
        theme: section.theme
        spacing: 10

        RowLayout {
            Layout.fillWidth: true
            spacing: 4
            visible: section.vitals.card_names.length > 1

            Repeater {
                model: section.vitals.card_names

                delegate: PillButton {
                    required property int index
                    required property string modelData

                    theme: section.theme
                    active: section.vitals.chosen_card === index
                    Layout.fillWidth: true
                    implicitHeight: 24
                    text: modelData
                    onClicked: section.vitals.choose_card(index)
                }
            }
        }

        Meter {
            theme: section.theme
            label: "GPU  ·  " + section.vitals.gpu_temperature_text
                   + "  ·  " + section.vitals.gpu_power_text
            value: section.vitals.gpu_usage_text
            fraction: section.vitals.gpu_usage
            past: section.vitals.gpu_history
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
