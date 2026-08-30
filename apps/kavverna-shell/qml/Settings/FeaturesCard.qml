import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../Shared"

/// Every utility in one place, grouped as the panel groups them. Removing one hides it from the
/// panel and from these settings without touching what it was configured to do, so putting it
/// back restores exactly that.
ColumnLayout {
    id: card

    required property var theme
    required property var features

    Layout.fillWidth: true
    spacing: card.theme.pad

    RowLayout {
        Layout.fillWidth: true

        SectionLabel {
            theme: card.theme
            text: "UTILITIES"
            Layout.fillWidth: true
        }

        Label {
            text: card.features.installed_count + " of " + card.features.built_count + " on"
            font.pixelSize: card.theme.textBody
            color: card.theme.secondaryText
        }
    }

    Card {
        theme: card.theme
        spacing: card.theme.gap

        Repeater {
            model: card.features.ids.length

            delegate: ColumnLayout {
                id: entry

                required property int index

                readonly property string id: card.features.ids[index]
                readonly property bool built: card.features.built[index]
                // The group name is only drawn when it changes, which turns a flat list into
                // the same five groups the panel has without a second model to keep in step.
                readonly property bool opensGroup: index === 0
                    || card.features.groups[index] !== card.features.groups[index - 1]

                Layout.fillWidth: true
                spacing: card.theme.gapSnug

                Label {
                    visible: entry.opensGroup
                    Layout.fillWidth: true
                    Layout.topMargin: entry.index === 0 ? 0 : card.theme.gapSnug
                    text: card.features.groups[entry.index].toUpperCase()
                    font.pixelSize: card.theme.textBody
                    font.bold: true
                    color: card.theme.mutedText
                }

                RowLayout {
                    Layout.fillWidth: true
                    spacing: card.theme.gap

                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 1

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: card.theme.gapSnug

                            Label {
                                text: card.features.titles[entry.index]
                                font.pixelSize: card.theme.textStrong
                                font.bold: true
                                color: entry.built ? card.theme.primaryText
                                                   : card.theme.mutedText
                                elide: Text.ElideRight
                            }

                            // What this utility keeps alive, so the choice to leave one on
                            // is made against what it costs rather than against a guess.
                            Label {
                                text: card.features.energies[entry.index]
                                font.pixelSize: card.theme.textBody
                                color: card.theme.mutedText
                                elide: Text.ElideRight
                            }

                            Item { Layout.fillWidth: true }
                        }

                        Label {
                            Layout.fillWidth: true
                            text: card.features.summaries[entry.index]
                            font.pixelSize: card.theme.textBody
                            color: card.theme.secondaryText
                            wrapMode: Text.WordWrap
                        }
                    }

                    Label {
                        visible: !entry.built
                        text: "On the way"
                        font.pixelSize: card.theme.textBody
                        color: card.theme.mutedText
                    }

                    Toggle {
                        id: entrySwitch
                        theme: card.theme
                        visible: entry.built
                        checked: card.features.installed[entry.index]
                        onToggled: card.features.choose_installed(entry.id, checked)
                        hoverEnabled: true

                        // An inside joke, reproduced verbatim; leave the text exactly as it
                        // is. Drawn in the window rather than as a ToolTip popup, which does
                        // not reliably appear over a layer surface.
                        Rectangle {
                            visible: entry.id === "themes" && entrySwitch.hovered
                            anchors.bottom: parent.top
                            anchors.bottomMargin: 6
                            anchors.right: parent.right
                            width: quip.implicitWidth + 16
                            height: quip.implicitHeight + 10
                            radius: card.theme.radiusSmall
                            color: card.theme.surface
                            border.width: 1
                            border.color: card.theme.hairline

                            Label {
                                id: quip
                                anchors.centerIn: parent
                                text: "Ian P. Mode ;-)"
                                font.pixelSize: card.theme.textSmall
                                color: card.theme.primaryText
                            }
                        }
                    }
                }
            }
        }
    }

    Label {
        Layout.fillWidth: true
        text: "A utility switched off here stops watching anything the next time Kavverna starts."
        font.pixelSize: card.theme.textBody
        color: card.theme.mutedText
        wrapMode: Text.WordWrap
    }
}
