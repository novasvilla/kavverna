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

                Layout.fillWidth: true
                spacing: 2

                RowLayout {
                    Layout.fillWidth: true
                    spacing: 6

                    Label {
                        text: outputRow.isDefault ? "\u25cf" : "\u25cb"
                        font.pixelSize: 10
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

                    Label {
                        text: section.mixer.output_volumes[outputRow.index] + "%"
                        font.pixelSize: section.theme.textBody
                        color: section.theme.secondaryText
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
                    value: section.mixer.output_volumes[outputRow.index]
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
                readonly property int percent: section.mixer.stream_volumes[index]

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

                    Label {
                        text: streamRow.percent + "%"
                        font.pixelSize: section.theme.textBody
                        color: streamRow.percent > 100 ? section.theme.warm
                                                       : section.theme.secondaryText
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
                    font.pixelSize: 10
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
    }
}
