import QtQuick
import QtQuick.Controls

/// A switch wearing the panel's own colours. The stock one follows the desktop's accent, which
/// left every switch Breeze blue in a panel that is otherwise lit by torchlight.
///
/// It never decides its own state: `on` is what the backend holds, and a click only asks for
/// the other value through `switched`. A stock Switch flips itself on the click, which destroys
/// a plain `checked:` binding, and a refused change then leaves it showing a value nobody holds:
/// the backend writes back the value it already had, and an unchanged property notifies nobody.
///
/// `text` is never drawn, the row beside the switch says it, but it is what a screen reader
/// announces, so every switch is given one.
Pressable {
    id: toggle

    /// What the backend says. The knob moves when this changes and at no other time.
    property bool on: false

    signal switched(bool wanted)

    implicitWidth: 40
    implicitHeight: 22
    corner: height / 2

    Accessible.role: Accessible.Switch
    Accessible.checkable: true
    Accessible.checked: toggle.on

    onClicked: toggle.switched(!toggle.on)

    contentItem: Rectangle {
        radius: height / 2
        color: toggle.on ? toggle.theme.accent : toggle.theme.control
        border.width: 1
        border.color: toggle.on ? toggle.theme.accent : toggle.theme.hairline
        opacity: toggle.enabled ? 1 : 0.4

        Rectangle {
            width: parent.height - 6
            height: width
            radius: width / 2
            y: 3
            x: toggle.on ? parent.width - width - 3 : 3
            // The knob has to read against whichever half of the track it is sitting on.
            color: toggle.on ? toggle.theme.surface : toggle.theme.secondaryText

            Behavior on x {
                NumberAnimation { duration: 110; easing.type: Easing.OutCubic }
            }
        }
    }
}
