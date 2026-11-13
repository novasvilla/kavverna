import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../Shared"

ColumnLayout {
    id: section

    required property var theme
    required property var vitals
    required property var agents
    required property var shows
    /// The applications behind each meter, and the ports they listen on.
    required property var processes
    required property var ports

    Layout.fillWidth: true
    spacing: section.theme.pad

    component Meter: ColumnLayout {
        id: meter

        required property var theme
        property string label
        property string value
        /// Which `open_fold` lists the applications behind this meter, empty for none.
        property string fold: ""
        property string foldTitle: ""
        property real fraction: 0
        property color tint: theme.accent
        /// The last couple of minutes of this reading. Left out for a meter whose past says
        /// nothing, such as how much of memory applications are holding.
        property var past: []

        Layout.fillWidth: true
        spacing: 3

        FoldHeading {
            theme: meter.theme
            label: meter.label
            value: meter.value
            foldable: meter.fold.length > 0
            open: meter.fold.length > 0 && section.processes.open_fold === meter.fold
            onClicked: section.processes.open_fold = open ? "" : meter.fold
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

        ApplicationFold {
            theme: meter.theme
            view: section.processes
            resource: meter.fold
            title: meter.foldTitle
        }
    }

    // First, because the processor card's traces make it tall and an allowance a person opens
    // the page to check should not sit below the fold.
    AgentUsageCard {
        theme: section.theme
        agents: section.agents
        visible: section.shows("agent-usage")
    }

    SectionLabel {
        theme: section.theme
        text: "PROCESSOR AND MEMORY"
        visible: section.shows("system-monitor")
    }

    Card {
        theme: section.theme
        spacing: section.theme.gap
        visible: section.shows("system-monitor")

        ColumnLayout {
            Layout.fillWidth: true
            visible: section.vitals.processor_name.length > 0
            spacing: 1

            LongLabel {
                theme: section.theme
                Layout.fillWidth: true
                text: section.vitals.processor_name
                font.pixelSize: section.theme.textBody
                font.bold: true
                color: section.theme.primaryText
                elide: Text.ElideRight
            }

            LongLabel {
                theme: section.theme
                Layout.fillWidth: true
                text: section.vitals.processor_detail
                font.pixelSize: section.theme.textFine
                color: section.theme.mutedText
                elide: Text.ElideRight
            }
        }

        Meter {
            theme: section.theme
            fold: "cpu"
            foldTitle: "Apps using CPU"
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
            fold: "memory"
            foldTitle: "Apps using memory"
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
        visible: section.shows("system-monitor")
    }

    Card {
        theme: section.theme
        spacing: section.theme.gap
        visible: section.shows("system-monitor")

        RowLayout {
            Layout.fillWidth: true
            spacing: section.theme.gapTight
            visible: section.vitals.card_names.length > 1

            Repeater {
                model: section.vitals.card_names

                delegate: PillButton {
                    required property int index
                    required property string modelData

                    theme: section.theme
                    active: section.vitals.chosen_card === index
                    Layout.fillWidth: true
                    compact: true
                    text: modelData
                    onClicked: section.vitals.choose_card(index)
                }
            }
        }

        // The GPU fold reads the card the picker shows, by the name the picker shows it under,
        // and is told before anything opens it, so one card's apps never land under another.
        Binding {
            target: section.processes
            property: "card_name"
            value: section.vitals.card_names[section.vitals.chosen_card] ?? ""
        }

        Meter {
            theme: section.theme
            fold: "gpu"
            foldTitle: "Apps using "
                       + (section.vitals.card_names[section.vitals.chosen_card] ?? "the GPU")
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

    SectionLabel {
        theme: section.theme
        text: "NETWORK"
        visible: section.shows("network-monitor")
    }

    Card {
        theme: section.theme
        spacing: section.theme.gap
        visible: section.shows("network-monitor")

        Label {
            Layout.fillWidth: true
            visible: section.vitals.network_status.length > 0
            text: section.vitals.network_status
            font.pixelSize: section.theme.textBody
            color: section.theme.secondaryText
            wrapMode: Text.WordWrap
        }

        Repeater {
            model: section.vitals.network_names.length

            delegate: ColumnLayout {
                id: interfaceRow
                required property int index
                Layout.fillWidth: true
                spacing: 3

                LongLabel {
                    theme: section.theme
                    Layout.fillWidth: true
                    text: section.vitals.network_names[interfaceRow.index]
                    font.pixelSize: section.theme.textStrong
                    font.bold: true
                    color: section.theme.primaryText
                }

                // Half the row each, so a fast link's longer figure cannot shove the other
                // direction off the card.
                RowLayout {
                    Layout.fillWidth: true
                    spacing: section.theme.gap

                    LongLabel {
                        theme: section.theme
                        Layout.fillWidth: true
                        Layout.preferredWidth: 1
                        text: section.vitals.network_received[interfaceRow.index]
                        font.pixelSize: section.theme.textBody
                        color: section.theme.secondaryText
                    }
                    LongLabel {
                        theme: section.theme
                        Layout.fillWidth: true
                        Layout.preferredWidth: 1
                        text: section.vitals.network_sent[interfaceRow.index]
                        font.pixelSize: section.theme.textBody
                        color: section.theme.secondaryText
                    }
                }

                LongLabel {
                    theme: section.theme
                    Layout.fillWidth: true
                    text: section.vitals.network_totals[interfaceRow.index]
                    font.pixelSize: section.theme.textSmall
                    color: section.theme.mutedText
                }
            }
        }
    }

    SectionLabel {
        theme: section.theme
        text: "STORAGE"
        visible: section.shows("system-monitor")
    }

    Card {
        theme: section.theme
        spacing: section.theme.gap
        visible: section.shows("system-monitor")

        Label {
            Layout.fillWidth: true
            visible: section.vitals.disk_status.length > 0
            text: section.vitals.disk_status
            font.pixelSize: section.theme.textBody
            color: section.theme.secondaryText
            wrapMode: Text.WordWrap
        }

        Repeater {
            model: section.vitals.disk_names.length

            delegate: ColumnLayout {
                id: diskRow
                required property int index
                Layout.fillWidth: true
                spacing: 3

                LongLabel {
                    theme: section.theme
                    Layout.fillWidth: true
                    text: section.vitals.disk_names[diskRow.index]
                    font.pixelSize: section.theme.textStrong
                    font.bold: true
                    color: section.theme.primaryText
                }
                LongLabel {
                    theme: section.theme
                    Layout.fillWidth: true
                    text: section.vitals.disk_spaces[diskRow.index]
                    font.pixelSize: section.theme.textBody
                    color: section.theme.secondaryText
                }
                LongLabel {
                    theme: section.theme
                    Layout.fillWidth: true
                    text: section.vitals.disk_rates[diskRow.index]
                    font.pixelSize: section.theme.textSmall
                    color: section.theme.mutedText
                }
            }
        }

        // One fold for the card rather than one per disk: the applications are counted by what
        // they read and write, not by which disk it lands on.
        FoldHeading {
            theme: section.theme
            label: "Disk activity"
            open: section.processes.open_fold === "disk"
            onClicked: section.processes.open_fold = open ? "" : "disk"
        }

        ApplicationFold {
            theme: section.theme
            view: section.processes
            resource: "disk"
            title: "Apps using the disk"
        }
    }

    PortsCard {
        theme: section.theme
        view: section.ports
        visible: section.shows("system-monitor")
    }
}
