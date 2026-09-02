import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.kde.layershell as LayerShell
import "../Shared"

/// The shelf itself: its own layer surface on the right edge, centred, so it outlives the
/// panel closing and never fights it for the corner. Everything dropped on it lands in one
/// deposit call; everything in it drags back out.
Window {
    id: shelfWindow

    required property var theme
    required property var shelf
    required property var shows
    /// Answers a screen by name, the panel's own lookup.
    required property var screenNamed

    /// Selected item ids, presentation state only. Click selects one; Ctrl+click grows it.
    property var picked: []

    // A mapped layer surface keeps the output it was created with, so a shelf that lands on
    // another screen, by a drag or because its own screen left, is made again there.
    property bool remapping: false

    function isPicked(id) {
        return shelfWindow.picked.indexOf(id) >= 0
    }

    /// One drop in, whatever surface caught it: the strip hands its drops here too.
    function receive(drop) {
        const formats = []
        for (let at = 0; at < drop.formats.length; at += 1) {
            formats.push(drop.formats[at])
        }
        const wanted = shelfWindow.shelf.wanted_format(formats.join("\n"))

        const urls = []
        for (let at = 0; at < drop.urls.length; at += 1) {
            urls.push(String(drop.urls[at]))
        }

        let moz = new ArrayBuffer(0)
        if (formats.indexOf("text/x-moz-url") >= 0) {
            moz = drop.getDataAsArrayBuffer("text/x-moz-url")
        }
        let imageBytes = new ArrayBuffer(0)
        if (wanted !== "" && urls.length === 0) {
            imageBytes = drop.getDataAsArrayBuffer(wanted)
        }

        shelfWindow.shelf.deposit(urls.join("\n"), drop.hasText ? drop.text : "",
                                  moz, imageBytes, wanted)
        drop.accept(Qt.CopyAction)
        shelfWindow.shelf.set_open(true)
    }

    function copyText(text) {
        copier.text = text
        copier.selectAll()
        copier.copy()
    }

    /// What the drag ghost shows: the shelf as it looked at the press.
    property var shelfShot: null

    width: 240
    height: Math.min(body.implicitHeight + 36, Math.min(640, Screen.desktopAvailableHeight - 24))
    visible: shelfWindow.shows("shelf") && shelfWindow.shelf.shelf_open && !shelfWindow.remapping
    color: "transparent"
    // Declared inside the panel's window, which would otherwise make this its transient
    // child, and a transient of a hidden window is never mapped.
    transientParent: null

    // A shelf stays where it was put. Never dragged, it hangs centred off the strip's edge.
    LayerShell.Window.anchors: shelfWindow.shelf.placed
        ? (LayerShell.Window.AnchorTop | LayerShell.Window.AnchorLeft)
        : (shelfWindow.shelf.strip_on_left ? LayerShell.Window.AnchorLeft
                                           : LayerShell.Window.AnchorRight)
    LayerShell.Window.layer: LayerShell.Window.LayerOverlay
    LayerShell.Window.margins: shelfWindow.shelf.placed
        ? Qt.rect(shelfWindow.shelf.shelf_left, shelfWindow.shelf.shelf_top, 0, 0)
        : (shelfWindow.shelf.strip_on_left ? Qt.rect(12, 0, 0, 0) : Qt.rect(0, 0, 12, 0))
    LayerShell.Window.keyboardInteractivity: LayerShell.Window.KeyboardInteractivityOnDemand
    LayerShell.Window.scope: "kavverna-shelf"
    // The same explicit screen pinning as the panel, for the same reason: the active screen
    // follows focus, and focus may live in a fullscreen game on another monitor.
    LayerShell.Window.screenConfiguration: LayerShell.Window.ScreenFromQWindow

    Binding on screen {
        when: shelfWindow.shelf.shelf_screen.length > 0
        value: shelfWindow.screenNamed(shelfWindow.shelf.shelf_screen)
    }

    Timer {
        id: remap
        interval: 40
        onTriggered: shelfWindow.remapping = false
    }

    Connections {
        target: shelfWindow.shelf
        function onShelf_screenChanged() { shelfWindow.replace() }
        function onShelf_leftChanged() { shelfWindow.replace() }
        function onShelf_topChanged() { shelfWindow.replace() }
    }

    /// Margins never move a mapped layer surface here, so a shelf that was dragged somewhere
    /// else is made again there. Never mid-drag: the ghost is what follows the hand.
    function replace() {
        if (shelfWindow.shelf.shelf_open && !shelfWindow.shelf.ghost_visible) {
            shelfWindow.remapping = true
            remap.restart()
        }
    }

    Shortcut {
        sequence: "Escape"
        onActivated: shelfWindow.shelf.set_open(false)
    }

    // The clipboard is reached through a text control because QML offers no other door to
    // it, and the shelf's own Wayland watcher must not be involved in writing.
    TextEdit {
        id: copier
        visible: false
    }

    Rectangle {
        id: shelfSkin
        anchors.fill: parent
        anchors.margins: 6
        radius: 14
        color: shelfWindow.theme.surface
        border.width: 1
        border.color: shelfWindow.theme.hairline
        // Blank while the ghost carries the portrait, so the drag reads as the shelf itself
        // moving; the surface stays mapped to keep the pointer grab alive.
        opacity: shelfWindow.shelf.ghost_visible && shelfWindow.shelfShot ? 0 : 1

        DropArea {
            anchors.fill: parent
            onDropped: (drop) => shelfWindow.receive(drop)
        }

        // The header is the shelf's handle, the same ghost gesture as the panel's.
        MouseArea {
            anchors.top: parent.top
            anchors.left: parent.left
            anchors.right: parent.right
            height: 44

            onPressed: (mouse) => {
                shelfSkin.grabToImage((shot) => shelfWindow.shelfShot = shot)
                const at = mapToItem(null, mouse.x, mouse.y)
                shelfWindow.shelf.drag_begun(Math.round(at.x), Math.round(at.y),
                                             shelfWindow.width, shelfWindow.height)
            }
            onPositionChanged: (mouse) => {
                if (!pressed) {
                    return
                }
                const at = mapToItem(null, mouse.x, mouse.y)
                shelfWindow.shelf.drag_preview(Math.round(at.x), Math.round(at.y),
                                               shelfWindow.width, shelfWindow.height)
            }
            onReleased: shelfWindow.shelf.drag_commit(shelfWindow.width, shelfWindow.height)
        }

        ColumnLayout {
            id: body
            anchors.fill: parent
            anchors.margins: shelfWindow.theme.pad
            spacing: shelfWindow.theme.gap

            RowLayout {
                Layout.fillWidth: true
                spacing: shelfWindow.theme.gapSnug

                Label {
                    text: "Shelf"
                    font.pixelSize: shelfWindow.theme.textStrong
                    font.bold: true
                    color: shelfWindow.theme.primaryText
                }

                Label {
                    Layout.fillWidth: true
                    text: shelfWindow.shelf.item_count > 0 ? shelfWindow.shelf.item_count : ""
                    font.pixelSize: shelfWindow.theme.textSmall
                    color: shelfWindow.theme.mutedText
                }

                IconButton {
                    theme: shelfWindow.theme
                    source: "edit-clear-all"
                    visible: shelfWindow.shelf.item_count > 0
                    ToolTip.visible: hovered
                    ToolTip.text: "Clear everything"
                    onClicked: {
                        shelfWindow.picked = []
                        shelfWindow.shelf.clear()
                    }
                }

                IconButton {
                    theme: shelfWindow.theme
                    source: "window-close"
                    onClicked: shelfWindow.shelf.set_open(false)
                }
            }

            Label {
                Layout.fillWidth: true
                visible: shelfWindow.shelf.notice.length > 0
                text: shelfWindow.shelf.notice
                font.pixelSize: shelfWindow.theme.textSmall
                color: shelfWindow.theme.warm
                wrapMode: Text.WordWrap
            }

            ColumnLayout {
                Layout.fillWidth: true
                visible: shelfWindow.shelf.item_count === 0
                spacing: shelfWindow.theme.gapSnug

                Label {
                    Layout.fillWidth: true
                    Layout.topMargin: 18
                    text: "Drop anything here"
                    horizontalAlignment: Text.AlignHCenter
                    font.pixelSize: shelfWindow.theme.textBody
                    color: shelfWindow.theme.secondaryText
                }

                Label {
                    Layout.fillWidth: true
                    Layout.bottomMargin: 18
                    text: "Files, text and links wait until they are dragged on. "
                          + "Ctrl+Alt+S opens this, mid-drag too."
                    horizontalAlignment: Text.AlignHCenter
                    font.pixelSize: shelfWindow.theme.textSmall
                    color: shelfWindow.theme.mutedText
                    wrapMode: Text.WordWrap
                }
            }

            ScrollView {
                Layout.fillWidth: true
                Layout.fillHeight: true
                visible: shelfWindow.shelf.item_count > 0
                contentWidth: availableWidth
                implicitHeight: rows.implicitHeight
                clip: true
                ScrollBar.horizontal.policy: ScrollBar.AlwaysOff

                ColumnLayout {
                    id: rows
                    width: parent.width
                    spacing: shelfWindow.theme.gapSnug

                    Repeater {
                        model: shelfWindow.shelf.row_ids.length

                        delegate: ShelfItem {
                            required property int index
                            Layout.fillWidth: true
                            theme: shelfWindow.theme
                            shelf: shelfWindow.shelf
                            home: shelfWindow
                            at: index
                        }
                    }
                }
            }
        }
    }
}
