import QtQuick
import QtQuick.Controls

/// A hint drawn inside the window rather than as a ToolTip popup, which does not reliably appear
/// over a layer surface. It lives in the window's overlay, so nothing laid out after its target
/// can cover it and no scrolling card clips it, and it is placed under the target when it opens,
/// or above it when there is no room below, kept inside the window either way.
Rectangle {
    id: bubble

    required property var theme
    required property Item target
    property bool shown: false
    property alias text: words.text

    parent: bubble.target.Overlay.overlay
    visible: bubble.shown && bubble.parent !== null
    z: 1000
    width: parent ? Math.min(natural.advanceWidth + 2 * theme.gap, parent.width - 2 * theme.pad)
                  : 0
    height: words.implicitHeight + 2 * theme.gapSnug
    radius: theme.radiusSmall
    color: theme.surface
    border.width: 1
    border.color: theme.hairline

    onShownChanged: if (shown) bubble.place()

    function place() {
        if (!bubble.parent) {
            return
        }
        const below = bubble.target.mapToItem(bubble.parent, 0, bubble.target.height)
        bubble.x = Math.max(bubble.theme.pad,
                            Math.min(below.x, bubble.parent.width - bubble.width - bubble.theme.pad))
        const under = below.y + bubble.theme.gapTight
        bubble.y = under + bubble.height <= bubble.parent.height - bubble.theme.pad
                 ? under
                 : bubble.target.mapToItem(bubble.parent, 0, 0).y - bubble.height - bubble.theme.gapTight
    }

    TextMetrics {
        id: natural
        font: words.font
        text: words.text
    }

    Label {
        id: words
        x: bubble.theme.gap
        y: bubble.theme.gapSnug
        width: bubble.width - 2 * bubble.theme.gap
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        font.pixelSize: bubble.theme.textSmall
        color: bubble.theme.primaryText
    }
}
