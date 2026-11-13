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
    /// Points per wheel notch. Volume is moved fast with the wheel, so a notch is a step a
    /// person can hear; the arrow keys keep the finer stepSize for landing on an exact figure.
    property real wheelStep: 5

    // The wheel over the slider itself changes the volume with no modifier; anywhere else on
    // the row it falls through and scrolls the page. Drags stay free because no snap mode is
    // set.
    wheelEnabled: false
    stepSize: 2

    // A touchpad reports a notch in many small deltas, so they are summed and a step is taken
    // for each whole notch, the remainder carried into the next event.
    property real wheelCarry: 0

    WheelHandler {
        target: null
        acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad
        onWheel: (event) => {
            const across = event.angleDelta.y === 0
            const delta = across ? -event.angleDelta.x
                                 : (event.inverted ? -event.angleDelta.y : event.angleDelta.y)
            level.wheelCarry += delta
            const notches = level.wheelCarry > 0 ? Math.floor(level.wheelCarry / 120)
                                                 : Math.ceil(level.wheelCarry / 120)
            if (notches === 0) {
                return
            }
            level.wheelCarry -= notches * 120
            const before = level.value
            const keyStep = level.stepSize
            // increase() and decrease() step the value without breaking the binding that
            // feeds it from the mixer, which a plain assignment would, so the step is widened
            // for the call rather than the value written.
            level.stepSize = Math.abs(notches) * level.wheelStep
            if (notches > 0) {
                level.increase()
            } else {
                level.decrease()
            }
            level.stepSize = keyStep
            // A drag reports moved and a step does not; the call sites listen for moved.
            if (level.value !== before) {
                level.moved()
            }
        }
    }

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
