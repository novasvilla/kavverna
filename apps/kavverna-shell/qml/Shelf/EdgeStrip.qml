import QtQuick
import org.kde.layershell as LayerShell
import "../Shared"

/// The honest version of a shelf that appears mid-drag: Wayland shows a client nothing about
/// a drag until it crosses the client's own surface, so a thin strip stays mapped on the
/// right edge and reacts the moment a drag enters it. It has to stay mapped; an unmapped
/// surface gets no drag events at all. Since a strip swallows the clicks that land on it
/// anyway, a click toggles the shelf.
Window {
    id: strip

    required property var theme
    required property var shelf
    required property var shows
    /// The shelf window, which handles any drop the strip itself catches.
    required property var home

    width: 16
    height: 220
    visible: strip.shows("shelf") && strip.shelf.edge_strip
    color: "transparent"
    // The same transient-of-a-hidden-panel trap the shelf window has.
    transientParent: null

    LayerShell.Window.anchors: LayerShell.Window.AnchorRight
    LayerShell.Window.layer: LayerShell.Window.LayerOverlay
    LayerShell.Window.keyboardInteractivity: LayerShell.Window.KeyboardInteractivityNone
    LayerShell.Window.scope: "kavverna-shelf-strip"
    LayerShell.Window.screenConfiguration: LayerShell.Window.ScreenFromQWindow

    Rectangle {
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        width: landing.containsDrag ? 14 : 6
        height: parent.height
        topLeftRadius: 6
        bottomLeftRadius: 6
        color: landing.containsDrag ? strip.theme.selected : strip.theme.glow
        border.width: 1
        border.color: strip.theme.hairline

        Behavior on width {
            NumberAnimation { duration: 110; easing.type: Easing.OutCubic }
        }
    }

    DropArea {
        id: landing
        anchors.fill: parent
        // Opening on entry lets the drag continue onto the shelf window that just appeared;
        // catching the drop here as well covers a compositor that does not deliver the drag
        // to a surface mapped mid-gesture.
        onEntered: strip.shelf.set_open(true)
        onDropped: (drop) => strip.home.receive(drop)
    }

    TapHandler {
        onTapped: strip.shelf.set_open(!strip.shelf.shelf_open)
    }
}
