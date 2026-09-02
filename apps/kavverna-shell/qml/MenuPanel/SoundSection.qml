import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import "../Shared"

ColumnLayout {
    id: section

    required property var theme
    required property var mixer
    /// Answers whether a utility is installed. The sound page hosts three, so the page staying
    /// when one of them is switched off is right, and each card closing itself is the rest of it.
    required property var shows

    Layout.fillWidth: true
    spacing: 12

    SectionLabel {
        theme: section.theme
        text: "OUTPUT"
        visible: section.shows("output-switcher")
    }

    Card {
        theme: section.theme
        spacing: 8
        visible: section.shows("output-switcher")

        Repeater {
            model: section.mixer.output_names.length

            delegate: ColumnLayout {
                id: outputRow
                required property int index
                readonly property bool isDefault:
                    section.mixer.output_ids[index] === section.mixer.default_output_id
                readonly property int percent:
                    index < section.mixer.output_volumes.length
                    ? section.mixer.output_volumes[index] : 0

                Layout.fillWidth: true
                spacing: 2

                RowLayout {
                    Layout.fillWidth: true
                    spacing: 6

                    Label {
                        text: outputRow.isDefault ? "\u25cf" : "\u25cb"
                        font.pixelSize: section.theme.textSmall
                        color: outputRow.isDefault ? section.theme.accent
                                                   : section.theme.secondaryText
                    }

                    Label {
                        Layout.fillWidth: true
                        text: section.mixer.output_names[outputRow.index]
                        font.pixelSize: section.theme.textBody
                        font.bold: outputRow.isDefault
                        color: section.theme.primaryText
                        elide: Text.ElideRight

                        TapHandler {
                            onTapped: section.mixer.make_default_output(
                                section.mixer.output_ids[outputRow.index])
                        }
                    }

                    FigureField {
                        theme: section.theme
                        value: outputRow.percent
                        maximum: 100
                        onCommitted: (value) => section.mixer.set_output_volume(
                            section.mixer.output_ids[outputRow.index], value)
                    }

                    IconButton {
                        theme: section.theme
                        source: section.mixer.output_muted[outputRow.index]
                                ? "audio-volume-muted" : "audio-volume-high"
                        onClicked: section.mixer.mute_output(
                            section.mixer.output_ids[outputRow.index],
                            !section.mixer.output_muted[outputRow.index])
                    }
                }

                Level {
                    theme: section.theme
                    Layout.fillWidth: true
                    from: 0
                    to: 100
                    value: outputRow.percent
                    onMoved: section.mixer.set_output_volume(
                        section.mixer.output_ids[outputRow.index], Math.round(value))
                }
            }
        }
    }

    SectionLabel {
        theme: section.theme
        text: "APPLICATIONS"
        visible: section.shows("volume-mixer")
    }

    Card {
        theme: section.theme
        spacing: 8
        visible: section.shows("volume-mixer")

        Label {
            visible: section.mixer.stream_names.length === 0
            text: "Nothing is playing"
            font.pixelSize: section.theme.textBody
            color: section.theme.secondaryText
        }

        Repeater {
            model: section.mixer.stream_names.length

            delegate: ColumnLayout {
                id: streamRow
                required property int index
                readonly property int percent:
                    index < section.mixer.stream_volumes.length
                    ? section.mixer.stream_volumes[index] : 0
                readonly property string anchorReason:
                    index < section.mixer.stream_anchors.length
                    ? section.mixer.stream_anchors[index] : ""
                readonly property int routedTo:
                    index < section.mixer.stream_route_device_ids.length
                    ? section.mixer.stream_route_device_ids[index] : -1
                property bool routeOpen: false

                /// Follow the default first, every output after, and the unplugged choice
                /// last so it stays visible instead of snapping back to Default.
                function routeChoices() {
                    const rows = [{ label: "Follow system default", value: -1 }]
                    for (let at = 0; at < section.mixer.output_names.length; at += 1) {
                        rows.push({ label: section.mixer.output_names[at],
                                    value: section.mixer.output_ids[at] })
                    }
                    if (streamRow.routedTo === -2) {
                        rows.push({ label: section.mixer.stream_route_labels[streamRow.index]
                                           + " (unplugged)", value: -2 })
                    }
                    return rows
                }

                Layout.fillWidth: true
                spacing: 2

                RowLayout {
                    Layout.fillWidth: true
                    spacing: 6

                    Kirigami.Icon {
                        source: section.mixer.stream_icons[streamRow.index]
                        implicitWidth: 16
                        implicitHeight: 16
                    }

                    Label {
                        Layout.fillWidth: true
                        text: section.mixer.stream_names[streamRow.index]
                        font.pixelSize: section.theme.textBody
                        color: section.theme.primaryText
                        elide: Text.ElideRight
                    }

                    FigureField {
                        theme: section.theme
                        value: streamRow.percent
                        maximum: 200
                        color: streamRow.percent > 100 ? section.theme.warm
                                                       : section.theme.secondaryText
                        onCommitted: (value) => section.mixer.set_stream_volume(
                            section.mixer.stream_ids[streamRow.index], value)
                    }
                }

                RowLayout {
                    Layout.fillWidth: true
                    spacing: 6

                    Level {
                        theme: section.theme
                        Layout.fillWidth: true
                        from: 0
                        to: 200
                        value: streamRow.percent
                        onMoved: section.mixer.set_stream_volume(
                            section.mixer.stream_ids[streamRow.index], Math.round(value))
                    }

                    IconButton {
                        theme: section.theme
                        source: section.mixer.stream_muted[streamRow.index]
                                ? "audio-volume-muted" : "audio-volume-high"
                        onClicked: section.mixer.mute_stream(
                            section.mixer.stream_ids[streamRow.index],
                            !section.mixer.stream_muted[streamRow.index])
                    }
                }

                Label {
                    Layout.fillWidth: true
                    visible: streamRow.anchorReason !== ""
                    text: streamRow.anchorReason
                    font.pixelSize: section.theme.textSmall
                    color: section.theme.mutedText
                }

                Label {
                    Layout.fillWidth: true
                    visible: streamRow.anchorReason === ""
                    text: "▸ " + section.mixer.stream_route_labels[streamRow.index]
                    font.pixelSize: section.theme.textSmall
                    color: streamRow.routeOpen ? section.theme.primaryText
                                               : section.theme.secondaryText
                    elide: Text.ElideRight

                    TapHandler {
                        onTapped: streamRow.routeOpen = !streamRow.routeOpen
                    }
                }

                Label {
                    Layout.fillWidth: true
                    visible: streamRow.anchorReason === "" && streamRow.routedTo === -2
                    text: "Using default until this device returns."
                    font.pixelSize: section.theme.textFine
                    color: section.theme.mutedText
                }

                ChoiceList {
                    visible: streamRow.routeOpen
                    theme: section.theme
                    choices: streamRow.routeChoices()
                    current: streamRow.routedTo
                    onPicked: (value) => {
                        const id = section.mixer.stream_ids[streamRow.index]
                        if (value === -1) {
                            section.mixer.route_stream_to_default(id)
                        } else {
                            section.mixer.route_stream(id, value)
                        }
                        streamRow.routeOpen = false
                    }
                }
            }
        }
    }

    SectionLabel {
        theme: section.theme
        text: "MICROPHONE"
        visible: section.shows("microphone-tools")
    }

    Card {
        theme: section.theme
        spacing: section.theme.gapSnug
        visible: section.shows("microphone-tools")

        RowLayout {
            Layout.fillWidth: true
            spacing: section.theme.gap

            Label {
                Layout.fillWidth: true
                text: section.mixer.inputs_muted
                      ? "Every microphone is muted"
                      : section.mixer.input_names.length + " available"
                font.pixelSize: section.theme.textBody
                color: section.theme.secondaryText
            }

            PillButton {
                theme: section.theme
                text: section.mixer.inputs_muted ? "Unmute all" : "Mute all"
                onClicked: section.mixer.mute_every_input(!section.mixer.inputs_muted)
            }
        }

        Repeater {
            model: section.mixer.input_names.length

            delegate: RowLayout {
                id: inputRow
                required property int index
                readonly property bool isDefault:
                    section.mixer.input_names[index] === section.mixer.default_input

                Layout.fillWidth: true
                spacing: section.theme.gapSnug

                Label {
                    text: inputRow.isDefault ? "\u25cf" : "\u25cb"
                    font.pixelSize: section.theme.textSmall
                    color: inputRow.isDefault ? section.theme.accent
                                              : section.theme.secondaryText
                }

                Label {
                    Layout.fillWidth: true
                    text: section.mixer.input_names[inputRow.index]
                    font.pixelSize: section.theme.textBody
                    font.bold: inputRow.isDefault
                    color: section.theme.primaryText
                    elide: Text.ElideRight

                    TapHandler {
                        onTapped: section.mixer.make_default_input(
                            section.mixer.input_ids[inputRow.index])
                    }
                }
            }
        }

        Label {
            visible: section.mixer.recorder_names.length > 0
            Layout.topMargin: section.theme.gapTight
            text: "Recording"
            font.pixelSize: section.theme.textStrong
            font.bold: true
            color: section.theme.primaryText
        }

        Repeater {
            model: section.mixer.recorder_names.length

            delegate: ColumnLayout {
                id: recorderRow
                required property int index
                readonly property string anchorReason:
                    index < section.mixer.recorder_anchors.length
                    ? section.mixer.recorder_anchors[index] : ""
                readonly property int routedTo:
                    index < section.mixer.recorder_route_device_ids.length
                    ? section.mixer.recorder_route_device_ids[index] : -1
                property bool routeOpen: false

                function sourceChoices() {
                    const rows = [{ label: "Follow system default", value: -1 }]
                    for (let at = 0; at < section.mixer.input_names.length; at += 1) {
                        rows.push({ label: section.mixer.input_names[at],
                                    value: section.mixer.input_ids[at] })
                    }
                    if (recorderRow.routedTo === -2) {
                        rows.push({ label: section.mixer.recorder_route_labels[recorderRow.index]
                                           + " (unplugged)", value: -2 })
                    }
                    return rows
                }

                Layout.fillWidth: true
                spacing: 2

                RowLayout {
                    Layout.fillWidth: true
                    spacing: 6

                    Kirigami.Icon {
                        source: section.mixer.recorder_icons[recorderRow.index]
                        implicitWidth: 16
                        implicitHeight: 16
                    }

                    Label {
                        Layout.fillWidth: true
                        text: section.mixer.recorder_names[recorderRow.index]
                        font.pixelSize: section.theme.textBody
                        color: section.theme.primaryText
                        elide: Text.ElideRight
                    }
                }

                Label {
                    Layout.fillWidth: true
                    visible: recorderRow.anchorReason !== ""
                    text: recorderRow.anchorReason
                    font.pixelSize: section.theme.textSmall
                    color: section.theme.mutedText
                }

                Label {
                    Layout.fillWidth: true
                    visible: recorderRow.anchorReason === ""
                    text: "▸ " + section.mixer.recorder_route_labels[recorderRow.index]
                    font.pixelSize: section.theme.textSmall
                    color: recorderRow.routeOpen ? section.theme.primaryText
                                                 : section.theme.secondaryText
                    elide: Text.ElideRight

                    TapHandler {
                        onTapped: recorderRow.routeOpen = !recorderRow.routeOpen
                    }
                }

                Label {
                    Layout.fillWidth: true
                    visible: recorderRow.anchorReason === "" && recorderRow.routedTo === -2
                    text: "Using default until this device returns."
                    font.pixelSize: section.theme.textFine
                    color: section.theme.mutedText
                }

                ChoiceList {
                    visible: recorderRow.routeOpen
                    theme: section.theme
                    choices: recorderRow.sourceChoices()
                    current: recorderRow.routedTo
                    onPicked: (value) => {
                        const id = section.mixer.recorder_ids[recorderRow.index]
                        if (value === -1) {
                            section.mixer.route_recorder_to_default(id)
                        } else {
                            section.mixer.route_recorder(id, value)
                        }
                        recorderRow.routeOpen = false
                    }
                }
            }
        }
    }
}
