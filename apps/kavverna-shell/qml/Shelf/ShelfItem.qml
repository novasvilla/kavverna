import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import "../Shared"

/// One shelved thing: thumbnail or icon, name and detail, actions on hover, and the drag
/// back out. Dragging a selected row takes the whole selection with it.
Rectangle {
    id: row

    required property var theme
    required property var shelf
    /// The shelf window, for the selection it keeps and the clipboard door it owns.
    required property var home
    required property int at

    readonly property int entryId: row.shelf.row_ids[at]
    readonly property bool alive: row.shelf.row_alive[at]
    readonly property bool onIt: row.home.isPicked(row.entryId)
    readonly property bool opensPile: row.shelf.row_pile_sizes[at] > 1
        && (row.at === 0 || row.shelf.row_pile_ids[at] !== row.shelf.row_pile_ids[at - 1])

    /// What the running drag carries, so the finish knows what left.
    property string draggedIds: ""
    /// A closer look at the item, read when the pointer arrives and shown under the row
    /// until it leaves or a drag begins. Inline rather than a popup: the row grows to hold
    /// it, so nothing floats over other rows and the pointer stays on the row it is reading.
    property string glance: ""
    readonly property bool glancing: hover.hovered && row.glance.length > 0

    implicitHeight: line.implicitHeight + 12
    radius: 8
    color: row.onIt ? row.theme.selected : hover.hovered ? row.theme.selected : row.theme.sunken
    border.width: row.onIt ? 1 : 0
    border.color: row.theme.accent

    Drag.dragType: Drag.Automatic
    Drag.supportedActions: Qt.CopyAction | Qt.MoveAction
    Drag.onDragFinished: (dropAction) => {
        if (dropAction !== Qt.IgnoreAction && row.draggedIds.length > 0) {
            row.home.picked = []
            row.shelf.taken(row.draggedIds)
        }
        row.draggedIds = ""
    }

    HoverHandler {
        id: hover
        onHoveredChanged: row.glance = hovered ? row.shelf.glance_of(row.entryId) : ""
    }

    MouseArea {
        anchors.fill: parent
        acceptedButtons: Qt.LeftButton
        property point pressAt

        onPressed: (mouse) => pressAt = Qt.point(mouse.x, mouse.y)

        onClicked: (mouse) => {
            if (mouse.modifiers & Qt.ControlModifier) {
                const grown = row.home.picked.slice()
                const here = grown.indexOf(row.entryId)
                if (here >= 0) {
                    grown.splice(here, 1)
                } else {
                    grown.push(row.entryId)
                }
                row.home.picked = grown
            } else {
                row.home.picked = row.onIt ? [] : [row.entryId]
            }
        }

        onPositionChanged: (mouse) => {
            if (!pressed || row.Drag.active) {
                return
            }
            if (Math.abs(mouse.x - pressAt.x) + Math.abs(mouse.y - pressAt.y) < 10) {
                return
            }
            // A drag of a selected row takes the selection; of an unselected one, itself.
            const ids = row.onIt && row.home.picked.length > 0
                      ? row.home.picked : [row.entryId]
            row.shelf.prepare_drag(ids.join(","))
            if (!row.shelf.drag_ok) {
                return
            }
            const mime = {}
            if (row.shelf.drag_uris.length > 0) {
                mime["text/uri-list"] = row.shelf.drag_uris
            }
            if (row.shelf.drag_text.length > 0) {
                mime["text/plain"] = row.shelf.drag_text
            }
            row.draggedIds = ids.join(",")
            row.Drag.mimeData = mime
            // The drag carries a picture of the row, the way a native file drag looks, and
            // begins through the property: startDrag() never started one on this platform.
            // The glance is folded away first so the picture is the row alone.
            row.glance = ""
            row.grabToImage((grabbed) => {
                row.Drag.imageSource = grabbed.url
                row.Drag.active = true
            })
        }
    }

    ColumnLayout {
        id: line
        anchors.fill: parent
        anchors.margins: 6
        spacing: 4

        Label {
            visible: row.opensPile
            text: row.shelf.row_pile_sizes[row.at] + " dropped together · tap to take all"
            font.pixelSize: row.theme.textFine
            color: row.theme.mutedText

            // Selecting the pile is how the whole drop gesture leaves together: pick it
            // here, then drag any of its rows.
            TapHandler {
                onTapped: {
                    const mine = row.shelf.row_pile_ids[row.at]
                    const all = []
                    for (let at = 0; at < row.shelf.row_ids.length; at += 1) {
                        if (row.shelf.row_pile_ids[at] === mine) {
                            all.push(row.shelf.row_ids[at])
                        }
                    }
                    row.home.picked = all
                }
            }
        }

        RowLayout {
            Layout.fillWidth: true
            spacing: row.theme.gapSnug

            Rectangle {
                implicitWidth: 36
                implicitHeight: 36
                radius: 6
                color: row.theme.sunken
                clip: true

                Image {
                    anchors.fill: parent
                    visible: row.shelf.row_thumbs[row.at] !== ""
                    source: row.shelf.row_thumbs[row.at]
                    sourceSize: Qt.size(64, 64)
                    fillMode: Image.PreserveAspectCrop
                    asynchronous: true
                }

                Kirigami.Icon {
                    anchors.centerIn: parent
                    visible: row.shelf.row_thumbs[row.at] === ""
                    source: row.shelf.row_icons[row.at]
                    implicitWidth: 22
                    implicitHeight: 22
                }
            }

            ColumnLayout {
                Layout.fillWidth: true
                spacing: 1

                Label {
                    Layout.fillWidth: true
                    text: row.shelf.row_names[row.at]
                    font.pixelSize: row.theme.textBody
                    color: row.alive ? row.theme.primaryText : row.theme.mutedText
                    elide: Text.ElideMiddle
                }

                Label {
                    Layout.fillWidth: true
                    visible: !hover.hovered
                    text: row.alive ? row.shelf.row_details[row.at] : "gone from disk"
                    font.pixelSize: row.theme.textFine
                    color: row.theme.mutedText
                    elide: Text.ElideRight
                }

                RowLayout {
                    Layout.fillWidth: true
                    visible: hover.hovered
                    spacing: 2

                    IconButton {
                        theme: row.theme
                        source: "document-open"
                        size: 11
                        implicitWidth: 22
                        implicitHeight: 22
                        enabled: row.alive
                        ToolTip.visible: hovered
                        ToolTip.text: "Open"
                        onClicked: Qt.openUrlExternally(row.shelf.open_target(row.entryId))
                    }

                    IconButton {
                        theme: row.theme
                        source: "system-file-manager"
                        size: 11
                        implicitWidth: 22
                        implicitHeight: 22
                        visible: row.shelf.row_kinds[row.at] !== "link"
                        enabled: row.alive
                        ToolTip.visible: hovered
                        ToolTip.text: "Reveal in the file manager"
                        onClicked: row.shelf.reveal(row.entryId)
                    }

                    IconButton {
                        theme: row.theme
                        source: "edit-copy"
                        size: 11
                        implicitWidth: 22
                        implicitHeight: 22
                        ToolTip.visible: hovered
                        ToolTip.text: row.shelf.row_kinds[row.at] === "link"
                                      ? "Copy the address" : "Copy the path"
                        onClicked: row.home.copyText(row.shelf.path_of(row.entryId))
                    }

                    Item { Layout.fillWidth: true }

                    IconButton {
                        theme: row.theme
                        source: "edit-delete"
                        size: 11
                        implicitWidth: 22
                        implicitHeight: 22
                        ToolTip.visible: hovered
                        ToolTip.text: "Take it off the shelf"
                        onClicked: {
                            row.home.picked = []
                            row.shelf.remove(row.entryId)
                        }
                    }
                }
            }
        }

        Image {
            id: closer
            Layout.fillWidth: true
            Layout.preferredHeight: Math.min(160, implicitHeight)
            visible: row.glancing && row.shelf.row_thumbs[row.at] !== ""
            source: visible ? row.shelf.row_thumbs[row.at] : ""
            sourceSize: Qt.size(400, 320)
            fillMode: Image.PreserveAspectFit
            asynchronous: true
        }

        Label {
            Layout.fillWidth: true
            visible: row.glancing
            text: row.glance
            // Whatever was dropped is shown as text, never rendered or fetched through.
            textFormat: Text.PlainText
            font.pixelSize: row.theme.textSmall
            color: row.theme.secondaryText
            wrapMode: Text.WrapAnywhere
            maximumLineCount: 8
            elide: Text.ElideRight
        }
    }
}
