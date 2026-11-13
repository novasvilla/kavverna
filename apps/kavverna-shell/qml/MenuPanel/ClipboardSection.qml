import QtQml
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import "../Shared"

ColumnLayout {
    id: section

    required property var theme
    required property var clipboard
    required property var shelf
    required property var shows

    // Which row the keyboard is on. Reset whenever the list changes, since the row that was
    // under the selection may no longer be there.
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
    spacing: section.theme.pad

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
        text: "SHELF"
        visible: section.shows("shelf")
    }

    Card {
        theme: section.theme
        visible: section.shows("shelf")

        RowLayout {
            Layout.fillWidth: true
            spacing: section.theme.gap

            Kirigami.Icon {
                source: "mail-attachment"
                implicitWidth: section.theme.iconLarge
                implicitHeight: section.theme.iconLarge
                isMask: true
                color: section.shelf.item_count > 0 ? section.theme.accent
                                                    : section.theme.secondaryText
            }

            Label {
                Layout.fillWidth: true
                text: section.shelf.item_count === 0
                      ? "Nothing waiting"
                      : section.shelf.item_count === 1
                        ? "One thing waiting"
                        : section.shelf.item_count + " things waiting"
                font.pixelSize: section.theme.textBody
                color: section.theme.secondaryText
                elide: Text.ElideRight
            }

            PillButton {
                theme: section.theme
                text: section.shelf.shelf_open ? "Close" : "Open"
                onClicked: section.shelf.set_open(!section.shelf.shelf_open)
            }
        }
    }

    SectionLabel {
        theme: section.theme
        text: "TRANSFORM"
        visible: section.shows("clipboard-transform")
    }

    Card {
        theme: section.theme
        visible: section.shows("clipboard-transform")
        spacing: section.theme.gapSnug

        RowLayout {
            Layout.fillWidth: true
            spacing: section.theme.gapSnug

            PillButton {
                theme: section.theme
                Layout.fillWidth: true
                text: "Plain"
                enabled: section.clipboard.can_transform
                onClicked: section.clipboard.transform(0)
            }

            PillButton {
                theme: section.theme
                Layout.fillWidth: true
                text: "JSON"
                enabled: section.clipboard.can_transform
                onClicked: section.clipboard.transform(1)
            }

            // Markdown needs the html of the copy itself, so it follows what the current
            // one offers rather than staying clickable and failing.
            PillButton {
                theme: section.theme
                Layout.fillWidth: true
                text: "Markdown"
                enabled: section.clipboard.can_markdown
                ToolTip.visible: hovered && !section.clipboard.can_markdown
                ToolTip.text: "This copy offers no HTML to convert"
                onClicked: section.clipboard.transform(2)
            }
        }

        Label {
            Layout.fillWidth: true
            text: section.clipboard.transform_notice.length > 0
                  ? section.clipboard.transform_notice
                  : "Shows the result first; the clipboard changes only on Use it, and the "
                    + "original stays in the history."
            font.pixelSize: section.theme.textSmall
            color: section.clipboard.transform_notice.length > 0
                   ? section.theme.primaryText : section.theme.secondaryText
            wrapMode: Text.WordWrap
        }

        Rectangle {
            Layout.fillWidth: true
            visible: section.clipboard.transform_preview.length > 0
            implicitHeight: Math.min(previewText.implicitHeight + 12, 150)
            radius: section.theme.radiusSmall
            color: section.theme.sunken
            clip: true

            Label {
                id: previewText
                anchors.fill: parent
                anchors.margins: 6
                text: section.clipboard.transform_preview
                // A previewed result is text, never markup to render or fetch through.
                textFormat: Text.PlainText
                font.family: "monospace"
                font.pixelSize: section.theme.textSmall
                color: section.theme.primaryText
                wrapMode: Text.WrapAnywhere
                maximumLineCount: 9
                elide: Text.ElideRight
            }
        }

        RowLayout {
            Layout.fillWidth: true
            visible: section.clipboard.transform_preview.length > 0
            spacing: section.theme.gapSnug

            PillButton {
                theme: section.theme
                Layout.fillWidth: true
                text: "Use it"
                onClicked: section.clipboard.use_transform()
            }

            PillButton {
                theme: section.theme
                Layout.fillWidth: true
                text: "Cancel"
                onClicked: section.clipboard.discard_transform()
            }
        }
    }

    SectionLabel {
        theme: section.theme
        text: "CLIPBOARD"
        visible: section.shows("clipboard-history")
    }

    Card {
        theme: section.theme
        visible: section.shows("clipboard-history")
        spacing: section.theme.gap

        Tick {
            theme: section.theme
            text: "Save clipboard history"
            on: section.clipboard.enabled
            onSwitched: (wanted) => section.clipboard.enable(wanted)
        }

        Label {
            Layout.fillWidth: true
            text: section.clipboard.enabled
                  ? "Everything stays on this machine and can be cleared at any time."
                  : "Turn this on to start saving what you copy."
            font.pixelSize: section.theme.textSmall
            color: section.theme.secondaryText
            wrapMode: Text.WordWrap
        }

        Label {
            Layout.fillWidth: true
            visible: section.shows("clean-url") && section.clipboard.clean_notice.length > 0
            text: section.clipboard.clean_notice
            font.pixelSize: section.theme.textSmall
            color: section.theme.accent
            wrapMode: Text.WordWrap
        }

        RowLayout {
            Layout.fillWidth: true
            spacing: section.theme.gapSnug
            visible: section.clipboard.enabled

            TextField {
                id: search
                Layout.fillWidth: true
                placeholderText: "Search copied text"
                placeholderTextColor: section.theme.mutedText
                font.pixelSize: section.theme.textBody
                color: section.theme.primaryText
                selectionColor: section.theme.selected
                selectedTextColor: section.theme.primaryText
                enabled: section.clipboard.row_ids.length > 0
                         || section.clipboard.query.length > 0
                onTextEdited: section.clipboard.search(text)
                // Opened by the shortcut, the first thing anyone does is type.
                onVisibleChanged: if (visible) forceActiveFocus()

                background: Rectangle {
                    radius: section.theme.radiusSmall
                    color: section.theme.sunken
                    border.width: search.activeFocus ? 1 : 0
                    border.color: section.theme.accent
                }
            }

            IconButton {
                theme: section.theme
                source: "edit-clear"
                size: section.theme.iconSmall
                tall: true
                visible: section.clipboard.query.length > 0
                text: "Clear the search"
                onClicked: {
                    search.text = "";
                    section.clipboard.search("");
                }
            }

            IconButton {
                theme: section.theme
                source: "edit-delete"
                size: section.theme.iconSmall
                tall: true
                enabled: section.clipboard.recent_count > 0
                text: "Clear everything not pinned"
                onClicked: section.clipboard.clear_unpinned()
            }
        }
    }

    Card {
        theme: section.theme
        visible: section.shows("clipboard-history") && section.clipboard.enabled
        implicitHeight: section.clipboard.row_ids.length > 0
                        ? Math.min(entries.implicitHeight + 24, 284)
                        : 72

        // An Item so the empty state can centre and the list can fill: the card lays its
        // children out in a padded column, and anchors need a plain parent.
        Item {
            Layout.fillWidth: true
            Layout.fillHeight: true

            Label {
                anchors.centerIn: parent
                visible: section.clipboard.row_ids.length === 0
                text: section.clipboard.query.length > 0 ? "No results" : "Nothing copied yet"
                font.pixelSize: section.theme.textSmall
                color: section.theme.mutedText
            }

            ScrollView {
                anchors.fill: parent
                visible: section.clipboard.row_ids.length > 0
                clip: true

            ColumnLayout {
                id: entries
                width: parent.width
                spacing: section.theme.gapSnug

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
                        radius: section.theme.radiusRow
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
                            spacing: section.theme.gapSnug

                            RowLayout {
                                Layout.fillWidth: true
                                spacing: section.theme.gapSnug

                                Kirigami.Icon {
                                    Layout.alignment: Qt.AlignTop
                                    visible: row.pinned
                                    source: "window-pin"
                                    implicitWidth: section.theme.iconSmall
                                    implicitHeight: section.theme.iconSmall
                                    isMask: true
                                    color: section.theme.accent
                                }

                                // Monochrome action icons rather than the mimetype ones: those
                                // are drawn in colour and flatten to a blob as a mask.
                                Kirigami.Icon {
                                    Layout.alignment: Qt.AlignTop
                                    source: row.kind === "image" ? "insert-image-symbolic"
                                          : row.kind === "files" ? "folder-symbolic"
                                                                 : "view-list-text"
                                    implicitWidth: section.theme.iconSmall
                                    implicitHeight: section.theme.iconSmall
                                    isMask: true
                                    color: section.theme.mutedText
                                }

                                Label {
                                    Layout.fillWidth: true
                                    text: section.clipboard.row_previews[row.index]
                                    // Copied text is text. Left on AutoText, a copied string
                                    // that looks like markup would be rendered, and anything it
                                    // pointed at would be fetched.
                                    textFormat: Text.PlainText
                                    font.pixelSize: section.theme.textSmall
                                    color: section.theme.primaryText
                                    wrapMode: Text.Wrap
                                    maximumLineCount: 3
                                    elide: Text.ElideRight
                                }
                            }

                            RowLayout {
                                Layout.fillWidth: true
                                spacing: section.theme.gapTight
                                visible: hover.hovered

                                IconButton {
                                    theme: section.theme
                                    source: "go-up"
                                    size: section.theme.iconSmall
                                    enabled: section.clipboard.query.length === 0
                                    text: "Move up"
                                    onClicked: section.clipboard.move_towards_top(row.entryId, true)
                                }

                                IconButton {
                                    theme: section.theme
                                    source: "go-down"
                                    size: section.theme.iconSmall
                                    enabled: section.clipboard.query.length === 0
                                    text: "Move down"
                                    onClicked: section.clipboard.move_towards_top(row.entryId, false)
                                }

                                IconButton {
                                    theme: section.theme
                                    source: row.pinned ? "window-pin" : "window-unpin"
                                    size: section.theme.iconSmall
                                    text: row.pinned ? "Unpin" : "Pin"
                                    onClicked: section.clipboard.pin(row.entryId, !row.pinned)
                                }

                                PillButton {
                                    theme: section.theme
                                    text: copied.running ? "Copied" : "Copy"
                                    compact: true
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

                                IconButton {
                                    theme: section.theme
                                    source: "edit-delete"
                                    size: section.theme.iconSmall
                                    text: "Delete"
                                    onClicked: section.clipboard.forget(row.entryId)
                                }
                            }

                            RowLayout {
                                Layout.fillWidth: true
                                visible: !hover.hovered
                                spacing: section.theme.gapSnug

                                Label {
                                    visible: row.index < 9
                                    text: "Ctrl+" + (row.index + 1)
                                    font.pixelSize: section.theme.textFine
                                    color: section.theme.mutedText
                                }

                                Item { Layout.fillWidth: true }

                                Label {
                                    text: new Date(section.clipboard.row_times[row.index])
                                          .toLocaleTimeString(Qt.locale(), Locale.ShortFormat)
                                    font.pixelSize: section.theme.textFine
                                    color: section.theme.mutedText
                                }
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
        visible: section.shows("clipboard-history") && section.clipboard.enabled
        spacing: section.theme.gapSnug

        Label {
            text: section.clipboard.pinned_count + " pinned  ·  "
                  + section.clipboard.recent_count + " recent"
            font.pixelSize: section.theme.textSmall
            color: section.theme.mutedText
        }

        Item { Layout.fillWidth: true }

        Label {
            visible: !section.clipboard.available
            text: "not watching"
            font.pixelSize: section.theme.textSmall
            color: section.theme.mutedText
        }
    }
}
