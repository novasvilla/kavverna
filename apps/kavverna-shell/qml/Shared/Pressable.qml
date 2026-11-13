import QtQuick
import QtQuick.Controls

/// Anything in the panel that answers a press: a tab, a device to make the default, a section
/// heading, a row to pick, and every button and switch built on top of it. One control so all
/// of them take the keyboard the same way: Tab reaches it, Space or Return presses it, a ring
/// shows where the keyboard is, the pointer becomes a hand over it, and a screen reader hears
/// what it is called. A Rectangle with a TapHandler offers none of that.
AbstractButton {
    id: pressable

    required property var theme
    /// What sits behind the content. The focus ring is drawn on the same shape.
    property color fill: "transparent"
    property real corner: theme.radiusSmall

    // The template computes no size of its own; a style normally does this.
    implicitWidth: Math.max(implicitBackgroundWidth + leftInset + rightInset,
                            implicitContentWidth + leftPadding + rightPadding)
    implicitHeight: Math.max(implicitBackgroundHeight + topInset + bottomInset,
                             implicitContentHeight + topPadding + bottomPadding)
    hoverEnabled: true
    focusPolicy: Qt.StrongFocus
    padding: 0

    Accessible.role: Accessible.Button
    Accessible.name: pressable.text

    // The clipboard page pastes the selected entry on Return through a window shortcut, and a
    // shortcut is offered the key before the focused item is. Claiming it here keeps Return for
    // whatever the keyboard is actually on.
    Keys.onShortcutOverride: (event) => event.accepted = pressable.activeFocus
        && (event.key === Qt.Key_Return || event.key === Qt.Key_Enter
            || event.key === Qt.Key_Space)
    Keys.onReturnPressed: pressable.click()
    Keys.onEnterPressed: pressable.click()

    HoverHandler {
        cursorShape: pressable.enabled ? Qt.PointingHandCursor : Qt.ArrowCursor
    }

    background: Rectangle {
        radius: pressable.corner
        color: pressable.fill
        // Only for the keyboard: a ring after every click would mark the last thing touched
        // rather than where Tab will go next.
        border.width: pressable.visualFocus ? pressable.theme.focusWidth : 0
        border.color: pressable.theme.focusRing
    }
}
