import QtQuick
import QtQuick.Controls

/// A number that reads as plain text until it is clicked, then becomes a field holding it
/// selected so the next keystroke replaces it. Enter applies, Escape puts the old figure back,
/// and focus moving to another control applies whatever whole number is there. A figure with
/// more digits than the maximum cannot be typed at all; one with the same number of digits but
/// past either end can, and Enter refuses it with an ember border until it is corrected.
Item {
    id: field

    required property var theme
    required property int value
    required property int maximum
    property int minimum: 0
    property string suffix: "%"
    property color color: theme.secondaryText
    signal committed(int value)

    property bool editing: false
    property bool refused: false

    implicitWidth: room.width + 2 * theme.gapSnug
    implicitHeight: 20

    TextMetrics {
        id: room
        font.pixelSize: field.theme.textBody
        text: field.maximum + field.suffix
    }

    Label {
        anchors.fill: parent
        visible: !field.editing
        text: field.value + field.suffix
        font.pixelSize: field.theme.textBody
        color: field.color
        horizontalAlignment: Text.AlignRight
        verticalAlignment: Text.AlignVCenter

        HoverHandler { cursorShape: Qt.IBeamCursor }
        TapHandler {
            onTapped: {
                editor.text = String(field.value)
                field.refused = false
                field.editing = true
                editor.forceActiveFocus()
                editor.selectAll()
            }
        }
    }

    TextField {
        id: editor
        anchors.fill: parent
        visible: field.editing
        leftPadding: 2
        rightPadding: sign.width + 3
        topPadding: 0
        bottomPadding: 0
        horizontalAlignment: Text.AlignRight
        verticalAlignment: Text.AlignVCenter
        font.pixelSize: field.theme.textBody
        color: field.theme.primaryText
        selectionColor: field.theme.selected
        selectedTextColor: field.theme.primaryText
        inputMethodHints: Qt.ImhDigitsOnly
        validator: IntValidator { bottom: field.minimum; top: field.maximum }

        // The panel's own Escape shortcut is offered the key first and would close the panel
        // mid-edit; claiming it here keeps the first Escape for the field.
        Keys.onShortcutOverride: (event) => event.accepted = event.key === Qt.Key_Escape
        Keys.onEscapePressed: {
            text = String(field.value)
            focus = false
        }
        // Return only reaches `accepted` for acceptable text, so the refusal is caught here.
        Keys.onReturnPressed: (event) => editor.settle(event)
        Keys.onEnterPressed: (event) => editor.settle(event)
        function settle(event) {
            if (acceptableInput) {
                focus = false
            } else {
                field.refused = true
            }
            event.accepted = true
        }
        onTextEdited: field.refused = false
        onActiveFocusChanged: if (!activeFocus) {
            if (acceptableInput && Number(text) !== field.value) {
                field.committed(Number(text))
            }
            field.editing = false
        }

        background: Rectangle {
            radius: field.theme.radiusSmall
            color: field.theme.sunken
            border.width: 1
            border.color: field.refused ? field.theme.ember : field.theme.accent
        }

        Label {
            id: sign
            anchors.right: parent.right
            anchors.rightMargin: 3
            anchors.verticalCenter: parent.verticalCenter
            text: field.suffix
            font.pixelSize: field.theme.textBody
            color: field.theme.secondaryText
        }
    }
}
