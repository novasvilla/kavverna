import QtQuick
import QtQuick.Controls

/// A line that may not fit: a device, an application, an interface or a mount point names
/// itself, and the panel is 360 wide. It elides to whatever room the layout gives it and the
/// whole of it can still be read, as its accessible name and, when the pointer rests on a line
/// that was cut, in a bubble. Give it a width, `Layout.fillWidth` or an explicit one: a label
/// left to its implicit width never elides, it just pushes the row past the panel's edge.
Label {
    id: label

    required property var theme

    elide: Text.ElideRight
    Accessible.name: label.text

    HoverHandler { id: pointer }

    Bubble {
        theme: label.theme
        target: label
        text: label.text
        shown: pointer.hovered && label.truncated
    }
}
