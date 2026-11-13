pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import "../Shared"

/// One entry of the clipboard history as a row of the list. The row itself is the pick: a click
/// or Return on it puts the entry back on the clipboard. Everything else it offers sits in a
/// slot of fixed height under the preview, so showing the actions never moves the rows below.
Pressable {
    id: row

    /// The ClipboardSection that owns the list: it holds which entry is open, since a reused
    /// delegate would otherwise carry that state over to whichever entry it shows next.
    required property var clipboardSection

    required property int index
    required property int entry_id
    required property string preview
    required property string kind
    required property bool pinned
    required property string copied_at_text
    required property url thumbnail
    required property int thumbnail_width
    required property int thumbnail_height
    required property bool selected

    readonly property var clipboard: clipboardSection.clipboard
    readonly property bool onIt: ListView.isCurrentItem
    readonly property bool isOpen: clipboardSection.openId === row.entry_id
    readonly property bool editable: row.kind === "text" || row.kind === "link"
    readonly property bool hasThumbnail: row.kind === "image" && row.thumbnail_width > 0
                                         && row.thumbnail_height > 0
    // Tab only reaches what is visible, so the actions show for the row the arrows are on and
    // for one the keyboard has walked into, as well as under the pointer.
    readonly property bool showsActions: hover.hovered || row.onIt || row.activeFocus
                                         || row.focusInside(actions)

    function focusInside(item) {
        for (let at = row.Window.activeFocusItem; at; at = at.parent) {
            if (at === item) {
                return true
            }
        }
        return false
    }

    theme: clipboardSection.theme
    width: ListView.view ? ListView.view.width : 0
    padding: 8
    corner: theme.radiusRow
    text: row.preview
    fill: hover.hovered || row.onIt ? theme.selected
        : row.selected ? theme.glow
                       : theme.sunken

    // A row that holds the keyboard was reached with Tab, and somebody pasting by keyboard is
    // about to press Ctrl+V: the panel gets out of the way at once.
    onClicked: clipboardSection.pick(row.entry_id, row.visualFocus)

    // Scrolled out of the list, the delegate goes back to the pool and may come back as another
    // entry. An edit in it would be left open with nothing on screen to close it.
    ListView.onPooled: if (row.isOpen) clipboardSection.closeOpen()

    HoverHandler { id: hover }

    // Over the content rather than behind it, so it is not hidden by the fill. It takes no
    // input, so the content still answers the pointer.
    Rectangle {
        anchors.fill: parent
        visible: row.onIt
        radius: row.corner
        color: "transparent"
        border.width: 1
        border.color: row.theme.accent
    }

    contentItem: ColumnLayout {
        spacing: row.theme.gapSnug

        RowLayout {
            Layout.fillWidth: true
            spacing: row.theme.gapSnug

            Item {
                Layout.alignment: Qt.AlignTop
                implicitWidth: row.hasThumbnail ? picture.width : row.theme.iconSmall
                implicitHeight: row.hasThumbnail ? row.theme.thumbnailHeight : row.theme.iconSmall

                Image {
                    id: picture
                    visible: row.hasThumbnail
                    source: row.hasThumbnail ? row.thumbnail : ""
                    height: row.theme.thumbnailHeight
                    width: row.hasThumbnail
                           ? Math.min(row.theme.thumbnailWidth,
                                      Math.round(height * row.thumbnail_width / row.thumbnail_height))
                           : 0
                    sourceSize.height: row.theme.thumbnailHeight * Screen.devicePixelRatio
                    fillMode: Image.PreserveAspectFit
                    asynchronous: true
                }

                // Monochrome action icons rather than the mimetype ones: those are drawn in
                // colour and flatten to a blob as a mask.
                Kirigami.Icon {
                    visible: !row.hasThumbnail
                    anchors.fill: parent
                    source: row.kind === "image" ? "insert-image-symbolic"
                          : row.kind === "files" ? "folder-symbolic"
                          : row.kind === "link" ? "insert-link-symbolic"
                                                : "view-list-text"
                    isMask: true
                    color: row.theme.mutedText
                }
            }

            // Its own press, so the text that was cut short can be read whole without putting
            // the entry back. With nothing more to show it is disabled, and a click on it falls
            // through to the row and picks like anywhere else.
            Pressable {
                id: previewArea
                Layout.fillWidth: true
                theme: row.theme
                enabled: row.kind !== "image"
                         && (row.isOpen || words.truncated || row.preview.endsWith("…"))
                text: row.isOpen ? "Show less" : "Show all of it"
                onClicked: row.isOpen ? row.clipboardSection.closeOpen()
                                      : row.clipboardSection.open(row.entry_id, false)

                contentItem: Label {
                    id: words
                    text: row.preview
                    // Copied text is text. Left on AutoText, a copied string that looks like
                    // markup would be rendered, and anything it pointed at would be fetched.
                    textFormat: Text.PlainText
                    font.pixelSize: row.theme.textSmall
                    color: row.theme.primaryText
                    wrapMode: Text.Wrap
                    maximumLineCount: 2
                    elide: Text.ElideRight
                }
            }

            Kirigami.Icon {
                Layout.alignment: Qt.AlignTop
                visible: row.selected
                source: "checkmark"
                implicitWidth: row.theme.iconSmall
                implicitHeight: row.theme.iconSmall
                isMask: true
                color: row.theme.accent
            }

            Kirigami.Icon {
                Layout.alignment: Qt.AlignTop
                visible: row.pinned
                source: "window-pin"
                implicitWidth: row.theme.iconSmall
                implicitHeight: row.theme.iconSmall
                isMask: true
                color: row.theme.accent
            }
        }

        // Made only while open, so a reused delegate never brings an editor or a reading
        // along from the entry it showed before.
        Loader {
            Layout.fillWidth: true
            active: row.isOpen
            visible: active
            sourceComponent: row.clipboardSection.openForEdit ? editor : wholeText
        }

        // The actions and the shortcut line share one slot as tall as the actions, so the row
        // is the same height whichever is showing and the list never moves under the pointer.
        // Hidden means invisible, not just transparent, so a hidden button takes no click or Tab.
        Item {
            Layout.fillWidth: true
            implicitHeight: row.theme.buttonHeightCompact

            RowLayout {
                id: actions
                anchors.fill: parent
                spacing: row.theme.gapTight
                opacity: row.showsActions ? 1 : 0
                Behavior on opacity { NumberAnimation { duration: row.theme.motionQuick } }
                visible: opacity > 0

                // Copies without closing the panel, for taking several things one after another.
                PillButton {
                    theme: row.theme
                    text: "Copy"
                    compact: true
                    onClicked: row.clipboard.put_back(row.entry_id)
                }

                IconButton {
                    theme: row.theme
                    source: row.pinned ? "window-unpin" : "window-pin"
                    size: row.theme.iconSmall
                    text: row.pinned ? "Unpin" : "Pin"
                    onClicked: row.clipboard.pin(row.entry_id, !row.pinned)
                }

                IconButton {
                    theme: row.theme
                    visible: row.editable
                    source: "document-edit"
                    size: row.theme.iconSmall
                    text: "Edit"
                    onClicked: row.clipboardSection.open(row.entry_id, true)
                }

                IconButton {
                    theme: row.theme
                    source: row.selected ? "checkbox-checked-symbolic" : "checkbox-symbolic"
                    size: row.theme.iconSmall
                    text: row.selected ? "Leave out of the selection" : "Select to paste together"
                    onClicked: row.clipboard.toggle_selected(row.entry_id)
                }

                IconButton {
                    theme: row.theme
                    source: "go-up"
                    size: row.theme.iconSmall
                    enabled: row.clipboard.query.length === 0
                    text: "Move up"
                    onClicked: row.clipboard.move_towards_top(row.entry_id, true)
                }

                IconButton {
                    theme: row.theme
                    source: "go-down"
                    size: row.theme.iconSmall
                    enabled: row.clipboard.query.length === 0
                    text: "Move down"
                    onClicked: row.clipboard.move_towards_top(row.entry_id, false)
                }

                Item { Layout.fillWidth: true }

                IconButton {
                    theme: row.theme
                    source: "edit-delete"
                    size: row.theme.iconSmall
                    text: "Delete"
                    onClicked: row.clipboard.forget(row.entry_id)
                }
            }

            RowLayout {
                anchors.fill: parent
                opacity: row.showsActions ? 0 : 1
                Behavior on opacity { NumberAnimation { duration: row.theme.motionQuick } }
                visible: opacity > 0
                spacing: row.theme.gapSnug

                Label {
                    visible: row.index < 9
                    text: "Ctrl+" + (row.index + 1)
                    font.pixelSize: row.theme.textFine
                    color: row.theme.mutedText
                }

                Item { Layout.fillWidth: true }

                Label {
                    text: row.copied_at_text
                    font.pixelSize: row.theme.textFine
                    color: row.theme.mutedText
                }
            }
        }
    }

    Component {
        id: wholeText

        Rectangle {
            id: whole

            readonly property bool ready: row.clipboard.full_text_ready
                                          && row.clipboard.full_text_id === row.entry_id

            // Ready with nothing in it means the entry went while it was being read.
            onReadyChanged: if (ready && row.clipboard.full_text.length === 0) row.clipboardSection.closeOpen()

            implicitHeight: Math.min(reading.implicitHeight, 10 * row.clipboardSection.lineHeight)
                            + 2 * row.theme.gapSnug
            radius: row.theme.radiusSmall
            color: row.theme.sunken

            // A Flickable takes the press, so a click inside the reading scrolls or does
            // nothing rather than falling through to the row and putting the entry back.
            Flickable {
                anchors.fill: parent
                anchors.margins: row.theme.gapSnug
                contentHeight: reading.implicitHeight
                clip: true
                boundsBehavior: Flickable.StopAtBounds
                ScrollBar.vertical: ScrollBar {}

                Label {
                    id: reading
                    width: parent.width
                    text: whole.ready ? row.clipboard.full_text : "Reading…"
                    textFormat: Text.PlainText
                    font.pixelSize: row.theme.textSmall
                    color: whole.ready ? row.theme.primaryText : row.theme.mutedText
                    wrapMode: Text.WrapAtWordBoundaryOrAnywhere
                }
            }
        }
    }

    Component {
        id: editor

        ColumnLayout {
            id: editing

            property bool filled: false
            property string refusal: ""

            spacing: row.theme.gapSnug

            // Filled once from the whole entry, never bound to it: a save writes the new text
            // back into the same property, and a binding would also overwrite what is typed.
            function fill() {
                if (editing.filled || !row.clipboard.full_text_ready
                        || row.clipboard.full_text_id !== row.entry_id) {
                    return
                }
                // Ready with nothing in it means the entry went while it was being read; the
                // notice says so, and an empty editor would only invite saving nothing.
                if (row.clipboard.full_text.length === 0) {
                    row.clipboardSection.closeOpen()
                    return
                }
                area.text = row.clipboard.full_text
                editing.filled = true
                area.cursorPosition = area.length
                area.forceActiveFocus()
            }

            function save() {
                const refusal = row.clipboard.rewrite(row.entry_id, area.text)
                if (refusal.length === 0) {
                    row.clipboardSection.closeOpen()
                } else {
                    editing.refusal = refusal
                }
            }

            Component.onCompleted: {
                row.clipboardSection.editor = editing
                editing.fill()
            }
            Component.onDestruction: if (row.clipboardSection.editor === editing) row.clipboardSection.editor = null

            Connections {
                target: row.clipboard
                function onFull_text_readyChanged() { editing.fill() }
            }

            ScrollView {
                Layout.fillWidth: true
                Layout.preferredHeight: Math.min(area.implicitHeight, 8 * row.clipboardSection.lineHeight
                                                 + area.topPadding + area.bottomPadding)

                TextArea {
                    id: area
                    readOnly: !editing.filled
                    placeholderText: editing.filled ? "" : "Reading…"
                    placeholderTextColor: row.theme.mutedText
                    textFormat: TextEdit.PlainText
                    wrapMode: TextEdit.Wrap
                    font.pixelSize: row.theme.textSmall
                    color: row.theme.primaryText
                    selectionColor: row.theme.selected
                    selectedTextColor: row.theme.primaryText
                    onTextChanged: editing.refusal = ""

                    // The window closes the panel on Escape, and a shortcut is offered the key
                    // before the focused item is. Claimed here, Escape cancels the edit
                    // instead of throwing the panel and the typing away together.
                    Keys.onShortcutOverride: (event) => event.accepted = area.activeFocus
                        && event.key === Qt.Key_Escape
                    Keys.onEscapePressed: row.clipboardSection.closeOpen()
                    Keys.onPressed: (event) => {
                        if ((event.key === Qt.Key_Return || event.key === Qt.Key_Enter)
                                && (event.modifiers & Qt.ControlModifier)) {
                            event.accepted = true
                            editing.save()
                        }
                    }

                    background: Rectangle {
                        radius: row.theme.radiusSmall
                        color: row.theme.sunken
                        border.width: area.activeFocus ? 1 : 0
                        border.color: row.theme.accent
                    }
                }
            }

            Label {
                Layout.fillWidth: true
                visible: editing.refusal.length > 0
                text: editing.refusal
                font.pixelSize: row.theme.textSmall
                color: row.theme.ember
                wrapMode: Text.WordWrap
            }

            RowLayout {
                Layout.fillWidth: true
                spacing: row.theme.gapSnug

                Label {
                    Layout.fillWidth: true
                    text: "Ctrl+Return saves"
                    font.pixelSize: row.theme.textFine
                    color: row.theme.mutedText
                    elide: Text.ElideRight
                }

                PillButton {
                    theme: row.theme
                    compact: true
                    text: "Cancel"
                    onClicked: row.clipboardSection.closeOpen()
                }

                PillButton {
                    theme: row.theme
                    compact: true
                    text: "Save"
                    enabled: editing.filled
                    onClicked: editing.save()
                }
            }
        }
    }
}
