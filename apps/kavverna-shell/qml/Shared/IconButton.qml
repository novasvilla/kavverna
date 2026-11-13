import QtQuick
import QtQuick.Controls
import org.kde.kirigami as Kirigami

/// A bare icon that answers a tap, drawn from the desktop's own theme rather than from a
/// character. A glyph is whatever font happens to be installed, and an emoji is somebody else's
/// colours in the middle of ours.
///
/// An icon says nothing by itself, so every one is given `text`: never drawn, but announced to
/// a screen reader and shown as the hover hint.
Pressable {
    id: button

    required property string source
    property int size: theme.iconMedium
    /// As tall as a PillButton, for one that sits in the same row as a text field or a pill.
    property bool tall: false

    implicitWidth: tall ? theme.buttonHeight : theme.buttonHeightCompact
    implicitHeight: implicitWidth
    fill: button.down ? button.theme.controlDown : "transparent"

    ToolTip.visible: button.hovered && button.text.length > 0
    ToolTip.text: button.text

    contentItem: Kirigami.Icon {
        source: button.source
        implicitWidth: button.size
        implicitHeight: button.size
        color: button.theme.secondaryText
        isMask: true
        opacity: button.enabled ? (button.hovered ? 1 : 0.75) : 0.35
    }
}
