import QtQml
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../Shared"

ColumnLayout {
    id: section

    required property var theme
    required property var clipboard

    /// Which row the keyboard is on. Reset whenever the list changes, since the row that was
    /// under the selection may no longer be there.
    property int selected: 0
    readonly property int rowCount: clipboard.row_ids.length

    signal picked()

    onRowCountChanged: selected = 0

    function choose(index) {
        if (index >= 0 && index < section.rowCount) {
            section.clipboard.put_back(section.clipboard.row_ids[index]);
            section.picked();
        }
    }

    Layout.fillWidth: true
    spacing: 12

    Shortcut {
        sequence: "Down"
        enabled: section.visible && section.rowCount > 0
        onActivated: section.selected = Math.min(section.selected + 1, section.rowCount - 1)
    }

    Shortcut {
        sequence: "Up"
        enabled: section.visible && section.rowCount > 0
        onActivated: section.selected = Math.max(section.selected - 1, 0)
    }

    Shortcut {
        sequences: ["Return", "Enter"]
        enabled: section.visible && section.rowCount > 0
        onActivated: section.choose(section.selected)
    }

    Instantiator {
        model: 9
        delegate: Shortcut {
            required property int index
            sequence: "Ctrl+" + (index + 1)
            enabled: section.visible
            onActivated: section.choose(index)
        }
    }

    SectionLabel {
        theme: section.theme
        text: "CLIPBOARD"
    }

    Card {
        theme: section.theme
        implicitHeight: controls.implicitHeight + 24

        ColumnLayout {
            id: controls
            anchors.fill: parent
            anchors.margins: 12
            spacing: 8

            CheckBox {
                id: keeping
                text: "Save clipboard history"
                checked: section.clipboard.enabled
                font.pixelSize: 12
                onToggled: section.clipboard.enable(checked)

                contentItem: Label {
                    text: keeping.text
                    font: keeping.font
                    color: section.theme.primaryText
                    leftPadding: keeping.indicator.width + 6
                    verticalAlignment: Text.AlignVCenter
                }
            }

            Label {
                Layout.fillWidth: true
                text: section.clipboard.enabled
                      ? "Everything stays on this machine and can be cleared at any time."
                      : "Turn this on to start saving what you copy."
                font.pixelSize: 10
                color: section.theme.secondaryText
                wrapMode: Text.WordWrap
            }

            RowLayout {
                Layout.fillWidth: true
                spacing: 6
                visible: section.clipboard.enabled

                TextField {
                    id: search
                    Layout.fillWidth: true
                    placeholderText: "Search copied text"
                    font.pixelSize: 11
                    color: section.theme.primaryText
                    enabled: section.clipboard.row_ids.length > 0
                             || section.clipboard.query.length > 0
                    onTextEdited: section.clipboard.search(text)
                    // Opened by the shortcut, the first thing anyone does is type.
                    onVisibleChanged: if (visible) forceActiveFocus()

                    background: Rectangle {
                        radius: 6
                        color: section.theme.sunken
                        border.width: search.activeFocus ? 1 : 0
                        border.color: section.theme.accent
                    }
                }

                Button {
                    icon.name: "edit-clear"
                    icon.width: 12
                    icon.height: 12
                    implicitWidth: 26
                    implicitHeight: 26
                    visible: section.clipboard.query.length > 0
                    ToolTip.visible: hovered
                    ToolTip.text: "Clear the search"
                    onClicked: {
                        search.text = "";
                        section.clipboard.search("");
                    }
                }

                Button {
                    icon.name: "edit-delete"
                    icon.width: 12
                    icon.height: 12
                    implicitWidth: 26
                    implicitHeight: 26
                    enabled: section.clipboard.recent_count > 0
                    ToolTip.visible: hovered
                    ToolTip.text: "Clear everything not pinned"
                    onClicked: section.clipboard.clear_unpinned()
                }
            }
        }
    }

    Card {
        theme: section.theme
        visible: section.clipboard.enabled
        implicitHeight: section.clipboard.row_ids.length > 0
                        ? Math.min(entries.implicitHeight + 24, 284)
                        : 72

        Label {
            anchors.centerIn: parent
            visible: section.clipboard.row_ids.length === 0
            text: section.clipboard.query.length > 0 ? "No results" : "Nothing copied yet"
            font.pixelSize: 10
            color: section.theme.mutedText
        }

        ScrollView {
            anchors.fill: parent
            anchors.margins: 12
            visible: section.clipboard.row_ids.length > 0
            clip: true

            ColumnLayout {
                id: entries
                width: parent.width
                spacing: 7

                Repeater {
                    model: section.clipboard.row_ids.length

                    delegate: Rectangle {
                        id: row
                        required property int index

                        readonly property int entryId: section.clipboard.row_ids[index]
                        readonly property bool pinned: section.clipboard.row_pinned[index]
                        readonly property string kind: section.clipboard.row_kinds[index]

                        readonly property bool onIt: section.selected === row.index

                        Layout.fillWidth: true
                        implicitHeight: line.implicitHeight + 16
                        radius: 8
                        color: hover.hovered || row.onIt ? section.theme.selected
                                                         : section.theme.sunken
                        border.width: row.onIt ? 1 : 0
                        border.color: section.theme.accent

                        HoverHandler { id: hover }
                        TapHandler { onTapped: section.choose(row.index) }

                        ColumnLayout {
                            id: line
                            anchors.fill: parent
                            anchors.margins: 8
                            spacing: 5

                            RowLayout {
                                Layout.fillWidth: true
                                spacing: 5

                                Label {
                                    visible: row.pinned
                                    text: "◆"
                                    font.pixelSize: 9
                                    color: section.theme.accent
                                }

                                Label {
                                    text: row.kind === "image" ? "▣"
                                        : row.kind === "files" ? "▫" : "≡"
                                    font.pixelSize: 10
                                    color: section.theme.mutedText
                                }

                                Label {
                                    Layout.fillWidth: true
                                    text: section.clipboard.row_previews[row.index]
                                    // Copied text is text. Left on AutoText, a copied string
                                    // that looks like markup would be rendered, and anything it
                                    // pointed at would be fetched.
                                    textFormat: Text.PlainText
                                    font.pixelSize: 10
                                    color: section.theme.primaryText
                                    wrapMode: Text.Wrap
                                    maximumLineCount: 3
                                    elide: Text.ElideRight
                                }
                            }

                            RowLayout {
                                Layout.fillWidth: true
                                spacing: 4
                                visible: hover.hovered

                                Button {
                                    icon.name: "go-up"
                                    icon.width: 11
                                    icon.height: 11
                                    implicitWidth: 22
                                    implicitHeight: 22
                                    enabled: section.clipboard.query.length === 0
                                    ToolTip.visible: hovered
                                    ToolTip.text: "Move up"
                                    onClicked: section.clipboard.move_towards_top(row.entryId, true)
                                }

                                Button {
                                    icon.name: "go-down"
                                    icon.width: 11
                                    icon.height: 11
                                    implicitWidth: 22
                                    implicitHeight: 22
                                    enabled: section.clipboard.query.length === 0
                                    ToolTip.visible: hovered
                                    ToolTip.text: "Move down"
                                    onClicked: section.clipboard.move_towards_top(row.entryId, false)
                                }

                                Button {
                                    text: row.pinned ? "◆" : "◇"
                                    implicitWidth: 22
                                    implicitHeight: 22
                                    font.pixelSize: 10
                                    ToolTip.visible: hovered
                                    ToolTip.text: row.pinned ? "Unpin" : "Pin"
                                    onClicked: section.clipboard.pin(row.entryId, !row.pinned)
                                }

                                Button {
                                    text: copied.running ? "Copied" : "Copy"
                                    implicitHeight: 22
                                    font.pixelSize: 10
                                    onClicked: {
                                        section.clipboard.put_back(row.entryId);
                                        copied.restart();
                                    }

                                    Timer {
                                        id: copied
                                        interval: 1200
                                    }
                                }

                                Item { Layout.fillWidth: true }

                                Button {
                                    icon.name: "edit-delete"
                                    icon.width: 11
                                    icon.height: 11
                                    implicitWidth: 22
                                    implicitHeight: 22
                                    ToolTip.visible: hovered
                                    ToolTip.text: "Delete"
                                    onClicked: section.clipboard.forget(row.entryId)
                                }
                            }

                            RowLayout {
                                Layout.fillWidth: true
                                visible: !hover.hovered
                                spacing: 6

                                Label {
                                    visible: row.index < 9
                                    text: "Ctrl+" + (row.index + 1)
                                    font.pixelSize: 9
                                    color: section.theme.mutedText
                                }

                                Item { Layout.fillWidth: true }

                                Label {
                                    text: new Date(section.clipboard.row_times[row.index])
                                          .toLocaleTimeString(Qt.locale(), Locale.ShortFormat)
                                    font.pixelSize: 9
                                    color: section.theme.mutedText
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    RowLayout {
        Layout.fillWidth: true
        visible: section.clipboard.enabled
        spacing: 6

        Label {
            text: section.clipboard.pinned_count + " pinned  ·  "
                  + section.clipboard.recent_count + " recent"
            font.pixelSize: 10
            color: section.theme.mutedText
        }

        Item { Layout.fillWidth: true }

        Label {
            visible: !section.clipboard.available
            text: "not watching"
            font.pixelSize: 10
            color: section.theme.mutedText
        }
    }
}
