pragma ComponentBehavior: Bound

import QtQml
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import "../Shared"

ColumnLayout {
    id: page

    required property var theme
    required property var clipboard
    required property var shelf
    required property var shows

    /// The entry whose whole text is open under its row, and whether for reading or for
    /// editing. Kept here, keyed by entry, because the list reuses its rows: state held in a
    /// row would follow the delegate to whatever entry it shows next. One at a time, since the
    /// view reads the full text of a single entry.
    property int openId: -1
    property bool openForEdit: false
    /// The editor while one is open. The list's keys stand down for it, so the arrows move the
    /// text cursor and Return breaks a line.
    property Item editor: null
    readonly property bool listShown: page.shows("clipboard-history")
                                      && page.clipboard.enabled
    readonly property int lineHeight: smallText.height

    signal picked()

    function open(id, forEdit) {
        page.clipboard.read_full(id)
        page.openForEdit = forEdit
        page.openId = id
    }

    function closeOpen() {
        page.openId = -1
        page.openForEdit = false
        if (page.visible && search.visible && search.enabled) {
            search.forceActiveFocus()
        }
    }

    /// The row at an index, made first if it is scrolled out of view, so a shortcut can reach
    /// an entry the list has not drawn.
    function rowAt(index) {
        if (index < 0 || index >= list.count) {
            return null
        }
        list.positionViewAtIndex(index, ListView.Contain)
        return list.itemAtIndex(index)
    }

    function step(by) {
        const next = Math.max(0, Math.min(list.currentIndex + by, list.count - 1))
        if (by > 0 && next === list.currentIndex && page.clipboard.has_more) {
            page.clipboard.show_more()
        }
        list.currentIndex = next
        list.positionViewAtIndex(next, ListView.Contain)
    }

    function choose(index) {
        const row = page.rowAt(index)
        if (row) {
            list.currentIndex = index
            page.pick(row.entry_id, true)
        }
    }

    function pick(id, byKeyboard) {
        page.clipboard.put_back(id)
        page.finish(byKeyboard)
    }

    /// Picking closes the panel for the paste that follows. With the pointer it stays a moment
    /// first, so the notice can say the entry is on the clipboard. With the keyboard it closes
    /// at once: the next key is Ctrl+V, and while the panel is still open that paste would land
    /// in the search field instead of where it was meant to go.
    function finish(byKeyboard) {
        // Nothing went anywhere, and the notice saying why has to stay on screen to be read.
        if (!page.clipboard.available) {
            return
        }
        if (byKeyboard) {
            page.picked()
        } else {
            linger.restart()
        }
    }

    Layout.fillWidth: true
    spacing: page.theme.pad

    // Every opening starts from the top: a list left scrolled from last time would hide the
    // newest entry, which is nearly always the one wanted. A close still pending from a pick
    // must not reach a panel that has since been opened again.
    onVisibleChanged: {
        linger.stop()
        page.closeOpen()
        if (visible) {
            list.currentIndex = 0
            list.positionViewAtBeginning()
        }
    }

    Timer {
        id: linger
        interval: 600
        onTriggered: page.picked()
    }

    FontMetrics {
        id: smallText
        font.pixelSize: page.theme.textSmall
    }

    // A new search is a new list, and a place in the old one means nothing there.
    Connections {
        target: page.clipboard
        function onModelReset() {
            page.closeOpen()
            list.currentIndex = 0
            list.positionViewAtBeginning()
        }
    }

    Shortcut {
        sequence: "Down"
        enabled: page.visible && page.editor === null && list.count > 0
        onActivated: page.step(1)
    }

    Shortcut {
        sequence: "Up"
        enabled: page.visible && page.editor === null && list.count > 0
        onActivated: page.step(-1)
    }

    Shortcut {
        sequences: ["Return", "Enter"]
        enabled: page.visible && page.editor === null && list.count > 0
        onActivated: page.choose(list.currentIndex)
    }

    Instantiator {
        model: 9
        delegate: Shortcut {
            required property int index
            sequence: "Ctrl+" + (index + 1)
            enabled: page.visible && page.editor === null
            onActivated: page.choose(index)
        }
    }

    SectionLabel {
        theme: page.theme
        text: "SHELF"
        visible: page.shows("shelf")
    }

    Card {
        theme: page.theme
        visible: page.shows("shelf")

        RowLayout {
            Layout.fillWidth: true
            spacing: page.theme.gap

            Kirigami.Icon {
                source: "mail-attachment"
                implicitWidth: page.theme.iconLarge
                implicitHeight: page.theme.iconLarge
                isMask: true
                color: page.shelf.item_count > 0 ? page.theme.accent
                                                    : page.theme.secondaryText
            }

            Label {
                Layout.fillWidth: true
                text: page.shelf.item_count === 0
                      ? "Nothing waiting"
                      : page.shelf.item_count === 1
                        ? "One thing waiting"
                        : page.shelf.item_count + " things waiting"
                font.pixelSize: page.theme.textBody
                color: page.theme.secondaryText
                elide: Text.ElideRight
            }

            PillButton {
                theme: page.theme
                text: page.shelf.shelf_open ? "Close" : "Open"
                onClicked: page.shelf.set_open(!page.shelf.shelf_open)
            }
        }
    }

    SectionLabel {
        theme: page.theme
        text: "TRANSFORM"
        visible: page.shows("clipboard-transform")
    }

    Card {
        theme: page.theme
        visible: page.shows("clipboard-transform")
        spacing: page.theme.gapSnug

        RowLayout {
            Layout.fillWidth: true
            spacing: page.theme.gapSnug

            PillButton {
                theme: page.theme
                Layout.fillWidth: true
                text: "Plain"
                enabled: page.clipboard.can_transform
                onClicked: page.clipboard.transform(0)
            }

            PillButton {
                theme: page.theme
                Layout.fillWidth: true
                text: "JSON"
                enabled: page.clipboard.can_transform
                onClicked: page.clipboard.transform(1)
            }

            // Markdown needs the html of the copy itself, so it follows what the current
            // one offers rather than staying clickable and failing.
            PillButton {
                theme: page.theme
                Layout.fillWidth: true
                text: "Markdown"
                enabled: page.clipboard.can_markdown
                ToolTip.visible: hovered && !page.clipboard.can_markdown
                ToolTip.text: "This copy offers no HTML to convert"
                onClicked: page.clipboard.transform(2)
            }
        }

        Label {
            Layout.fillWidth: true
            text: page.clipboard.transform_notice.length > 0
                  ? page.clipboard.transform_notice
                  : "Shows the result first; the clipboard changes only on Use it, and the "
                    + "original stays in the history."
            font.pixelSize: page.theme.textSmall
            color: page.clipboard.transform_notice.length > 0
                   ? page.theme.primaryText : page.theme.secondaryText
            wrapMode: Text.WordWrap
        }

        Rectangle {
            Layout.fillWidth: true
            visible: page.clipboard.transform_preview.length > 0
            implicitHeight: Math.min(previewText.implicitHeight + 12, 150)
            radius: page.theme.radiusSmall
            color: page.theme.sunken
            clip: true

            Label {
                id: previewText
                anchors.fill: parent
                anchors.margins: 6
                text: page.clipboard.transform_preview
                // A previewed result is text, never markup to render or fetch through.
                textFormat: Text.PlainText
                font.family: "monospace"
                font.pixelSize: page.theme.textSmall
                color: page.theme.primaryText
                wrapMode: Text.WrapAnywhere
                maximumLineCount: 9
                elide: Text.ElideRight
            }
        }

        RowLayout {
            Layout.fillWidth: true
            visible: page.clipboard.transform_preview.length > 0
            spacing: page.theme.gapSnug

            PillButton {
                theme: page.theme
                Layout.fillWidth: true
                text: "Use it"
                onClicked: page.clipboard.use_transform()
            }

            PillButton {
                theme: page.theme
                Layout.fillWidth: true
                text: "Cancel"
                onClicked: page.clipboard.discard_transform()
            }
        }
    }

    SectionLabel {
        theme: page.theme
        text: "CLIPBOARD"
        visible: page.shows("clipboard-history")
    }

    Card {
        theme: page.theme
        visible: page.shows("clipboard-history")
        spacing: page.theme.gap

        SettingRow {
            theme: page.theme
            title: "Save clipboard history"
            detail: page.clipboard.enabled
                    ? "Everything stays on this machine and can be cleared at any time."
                    : "History is off, so nothing you copy is kept. Turn it on to start saving."
            on: page.clipboard.enabled
            onToggled: (value) => page.clipboard.enable(value)
        }

        Label {
            Layout.fillWidth: true
            visible: page.shows("clean-url") && page.clipboard.clean_notice.length > 0
            text: page.clipboard.clean_notice
            font.pixelSize: page.theme.textSmall
            color: page.theme.accent
            wrapMode: Text.WordWrap
        }

        RowLayout {
            Layout.fillWidth: true
            spacing: page.theme.gapSnug
            visible: page.clipboard.enabled

            TextField {
                id: search
                Layout.fillWidth: true
                placeholderText: "Search copied text"
                placeholderTextColor: page.theme.mutedText
                font.pixelSize: page.theme.textBody
                color: page.theme.primaryText
                selectionColor: page.theme.selected
                selectedTextColor: page.theme.primaryText
                enabled: list.count > 0 || page.clipboard.query.length > 0
                onTextEdited: page.clipboard.search(text)
                // Opened by the shortcut, the first thing anyone does is type.
                onVisibleChanged: if (visible) forceActiveFocus()

                background: Rectangle {
                    radius: page.theme.radiusSmall
                    color: page.theme.sunken
                    border.width: search.activeFocus ? 1 : 0
                    border.color: page.theme.accent
                }
            }

            IconButton {
                theme: page.theme
                source: "edit-clear"
                size: page.theme.iconSmall
                tall: true
                visible: page.clipboard.query.length > 0
                text: "Clear the search"
                onClicked: {
                    search.text = "";
                    page.clipboard.search("");
                }
            }

            ConfirmButton {
                theme: page.theme
                source: "edit-delete"
                tall: true
                enabled: page.clipboard.recent_count > 0
                text: "Clear everything not pinned"
                confirmText: "Clear " + page.clipboard.recent_count + "?"
                onConfirmed: page.clipboard.clear_unpinned()
            }
        }
    }

    Card {
        theme: page.theme
        visible: page.listShown
        implicitHeight: list.count > 0
                        ? Math.min(list.contentHeight + 2 * page.theme.pad,
                                   page.theme.clipboardListHeight)
                        : page.theme.clipboardEmptyHeight

        // An Item so the empty state can centre and the list can fill: the card lays its
        // children out in a padded column, and anchors need a plain parent.
        Item {
            Layout.fillWidth: true
            Layout.fillHeight: true

            Label {
                anchors.centerIn: parent
                width: parent.width
                visible: list.count === 0
                horizontalAlignment: Text.AlignHCenter
                text: !page.clipboard.available
                      ? "The clipboard is not being read, so nothing new is saved."
                      : page.clipboard.query.length > 0 ? "No entry matches"
                                                           : "Nothing copied yet"
                font.pixelSize: page.theme.textSmall
                color: page.theme.mutedText
                wrapMode: Text.WordWrap
            }

            // The list scrolls inside the card, so the wheel over it moves the entries and not
            // the page. Rows are made for what is on screen, reused as it scrolls, and the next
            // page of the history is asked for as the end comes near.
            ListView {
                id: list
                anchors.fill: parent
                visible: list.count > 0
                model: page.clipboard
                reuseItems: true
                clip: true
                spacing: page.theme.gapSnug
                boundsBehavior: Flickable.StopAtBounds
                // The arrows belong to the window's shortcuts, which work from the search field.
                keyNavigationEnabled: false
                ScrollBar.vertical: ScrollBar {}

                // After a long scroll down the window slides, leaving the top of the history
                // behind; this brings it back.
                header: Item {
                    width: list.width
                    height: page.clipboard.has_earlier
                            ? earlier.implicitHeight + page.theme.gapSnug : 0
                    visible: page.clipboard.has_earlier

                    PillButton {
                        id: earlier
                        width: parent.width
                        theme: page.theme
                        compact: true
                        text: "Show the entries above"
                        onClicked: page.clipboard.show_earlier()
                    }
                }

                delegate: ClipboardEntry {
                    clipboardSection: page
                }
            }

            // Over the list rather than in the column, so a notice coming and going never
            // moves a row out from under the pointer.
            Rectangle {
                id: noticePill

                property string said: ""

                anchors.horizontalCenter: parent.horizontalCenter
                anchors.bottom: parent.bottom
                width: Math.min(saying.implicitWidth + 2 * page.theme.gap, parent.width)
                height: saying.implicitHeight + 2 * page.theme.gapTight
                radius: height / 2
                color: page.theme.surface
                border.width: 1
                border.color: page.theme.hairline
                opacity: page.clipboard.notice.length > 0 ? 1 : 0
                visible: opacity > 0
                Behavior on opacity { NumberAnimation { duration: page.theme.motionQuick } }

                // The notice empties itself after two seconds; the last words stay while the
                // pill fades rather than vanishing from it first.
                Connections {
                    target: page.clipboard
                    function onNoticeChanged() {
                        if (page.clipboard.notice.length > 0) {
                            noticePill.said = page.clipboard.notice
                        }
                    }
                }

                Label {
                    id: saying
                    anchors.centerIn: parent
                    width: Math.min(implicitWidth, noticePill.parent.width - 2 * page.theme.gap)
                    text: noticePill.said
                    textFormat: Text.PlainText
                    font.pixelSize: page.theme.textSmall
                    color: page.theme.primaryText
                    elide: Text.ElideRight
                }
            }
        }
    }

    // The selection is told here, under the list in the line the counts use, rather than in a
    // bar above it: a bar arriving there on the first pick would push every row down, and the
    // next click meant for one row would land on the row after it.
    Item {
        Layout.fillWidth: true
        visible: page.listShown
        implicitHeight: page.theme.buttonHeight

        RowLayout {
            anchors.fill: parent
            visible: page.clipboard.selected_count === 0
            spacing: page.theme.gapSnug

            Label {
                text: page.clipboard.pinned_count + " pinned  ·  "
                      + page.clipboard.recent_count + " recent"
                font.pixelSize: page.theme.textSmall
                color: page.theme.mutedText
            }

            Item { Layout.fillWidth: true }
        }

        RowLayout {
            anchors.fill: parent
            visible: page.clipboard.selected_count > 0
            spacing: page.theme.gapSnug

            Label {
                Layout.fillWidth: true
                text: page.clipboard.selected_count + " selected"
                font.pixelSize: page.theme.textSmall
                color: page.theme.primaryText
                elide: Text.ElideRight
            }

            PillButton {
                id: pasteTogether
                theme: page.theme
                text: "Paste together"
                onClicked: {
                    page.clipboard.paste_selection()
                    // The selection is let go only once the joined text is on its way, so one
                    // still standing means nothing reached the clipboard.
                    if (page.clipboard.selected_count === 0) {
                        page.finish(pasteTogether.visualFocus)
                    }
                }
            }

            PillButton {
                theme: page.theme
                text: "Clear"
                onClicked: page.clipboard.clear_selection()
            }
        }
    }
}
