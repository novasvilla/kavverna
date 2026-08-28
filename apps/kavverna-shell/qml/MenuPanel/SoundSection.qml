import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../Shared"

ColumnLayout {
    id: section

    required property var theme
    required property var mixer

    Layout.fillWidth: true
    spacing: 12

    SectionLabel {
        theme: section.theme
        text: "OUTPUT"
    }

    Card {
        theme: section.theme
        implicitHeight: outputs.implicitHeight + section.theme.pad * 2

        ColumnLayout {
            id: outputs
            anchors.fill: parent
            anchors.margins: section.theme.pad
            spacing: 8

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
                    }

                    Slider {
                        Layout.fillWidth: true
                        implicitHeight: 18
                        from: 0
                        to: 100
                        value: section.mixer.output_volumes[outputRow.index]
                        onMoved: section.mixer.set_output_volume(
                            section.mixer.output_ids[outputRow.index], Math.round(value))
                    }
                }
            }
        }
    }

    SectionLabel {
        theme: section.theme
        text: "APPLICATIONS"
    }

    Card {
        theme: section.theme
        implicitHeight: streams.implicitHeight + section.theme.pad * 2

        ColumnLayout {
            id: streams
            anchors.fill: parent
            anchors.margins: section.theme.pad
            spacing: 8

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

                        Slider {
                            Layout.fillWidth: true
                            implicitHeight: 18
                            from: 0
                            to: 200
                            value: streamRow.percent
                            onMoved: section.mixer.set_stream_volume(
                                section.mixer.stream_ids[streamRow.index], Math.round(value))
                        }

                        Label {
                            text: section.mixer.stream_muted[streamRow.index] ? "\ud83d\udd07"
                                                                              : "\ud83d\udd0a"
                            font.pixelSize: 12

                            TapHandler {
                                onTapped: section.mixer.mute_stream(
                                    section.mixer.stream_ids[streamRow.index],
                                    !section.mixer.stream_muted[streamRow.index])
                            }
                        }
                    }
                }
            }
        }
    }

    SectionLabel {
        theme: section.theme
        text: "MICROPHONE"
    }

    Card {
        theme: section.theme
        implicitHeight: microphone.implicitHeight + section.theme.pad * 2

        RowLayout {
            id: microphone
            anchors.fill: parent
            anchors.margins: section.theme.pad
            spacing: 10

            ColumnLayout {
                Layout.fillWidth: true
                spacing: 1

                Label {
                    Layout.fillWidth: true
                    text: section.mixer.default_input
                    font.pixelSize: 12
                    font.bold: true
                    color: section.theme.primaryText
                    elide: Text.ElideRight
                }

                Label {
                    text: section.mixer.inputs_muted
                          ? "Every microphone is muted"
                          : section.mixer.input_names.length + " inputs"
                    font.pixelSize: section.theme.textBody
                    color: section.theme.secondaryText
                }
            }

            Button {
                text: section.mixer.inputs_muted ? "Unmute all" : "Mute all"
                font.pixelSize: section.theme.textBody
                implicitHeight: 26
                onClicked: section.mixer.mute_every_input(!section.mixer.inputs_muted)
            }
        }
    }
}
