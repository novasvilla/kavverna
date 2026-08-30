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

    /// Selected item ids, presentation state only. Click selects one; Ctrl+click grows it.
    property var picked: []

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

    width: 240
    height: Math.min(body.implicitHeight + 36, Math.min(640, Screen.desktopAvailableHeight - 24))
    visible: shelfWindow.shows("shelf") && shelfWindow.shelf.shelf_open
    color: "transparent"
    // Declared inside the panel's window, which would otherwise make this its transient
    // child, and a transient of a hidden window is never mapped.
    transientParent: null

    LayerShell.Window.anchors: LayerShell.Window.AnchorRight
    LayerShell.Window.layer: LayerShell.Window.LayerOverlay
    LayerShell.Window.margins: Qt.rect(0, 0, 12, 0)
    LayerShell.Window.keyboardInteractivity: LayerShell.Window.KeyboardInteractivityOnDemand
    LayerShell.Window.scope: "kavverna-shelf"
    // The same primary-screen pinning as the panel, for the same reason: the active screen
    // follows focus, and focus lives in the fullscreen game on the other monitor.
    LayerShell.Window.screenConfiguration: LayerShell.Window.ScreenFromQWindow

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
        anchors.fill: parent
        anchors.margins: 6
        radius: 14
        color: shelfWindow.theme.surface
        border.width: 1
        border.color: shelfWindow.theme.hairline

        DropArea {
            anchors.fill: parent
            onDropped: (drop) => shelfWindow.receive(drop)
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
