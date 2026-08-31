import QtQuick
import QtQuick.Controls

/// A volume slider in the panel's colours. Past a hundred the fill turns from torchlight to
/// ember, because boost is amplification and it should look like a warning rather than like
/// more of the same.
Slider {
    id: level

    required property var theme
    /// Where the track stops meaning louder and starts meaning louder than the source.
    property real unity: to > 100 ? 100 : to

    implicitHeight: 18
    // The wheel adjusts rather than scrolls while over a slider. Two per notch: fine enough
    // to land on an exact figure, quick enough to cross the range. Drags stay free because
    // no snap mode is set.
    wheelEnabled: true
    stepSize: 2

    background: Rectangle {
        x: level.leftPadding
        y: level.topPadding + level.availableHeight / 2 - height / 2
        width: level.availableWidth
        height: 4
        radius: 2
        color: level.theme.control

        Rectangle {
            width: level.visualPosition * parent.width
            height: parent.height
            radius: parent.radius
            color: level.value > level.unity ? level.theme.ember : level.theme.accent
        }
    }

    handle: Rectangle {
        x: level.leftPadding + level.visualPosition * (level.availableWidth - width)
        y: level.topPadding + level.availableHeight / 2 - height / 2
        implicitWidth: 14
        implicitHeight: 14
        radius: width / 2
        color: level.pressed ? level.theme.controlDown : level.theme.primaryText
        border.width: 1
        border.color: level.theme.hairline
    }
}
