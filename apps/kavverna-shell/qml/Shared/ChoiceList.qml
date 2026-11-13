import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

/// The output card's marker-and-label rows as a reusable column: a list to pick one thing
/// from, opened in place. A popup would fight a 360 wide panel for room; a column that grows
/// its card does not.
ColumnLayout {
    id: list

    required property var theme
    /// [{ label, value }] rows. A value of -2 is drawn but not pickable: the entry for a
    /// chosen device that is unplugged, kept visible so the choice never looks forgotten.
    required property var choices
    required property int current

    signal picked(int value)

    Layout.fillWidth: true
    spacing: theme.gapTight

    Repeater {
        model: list.choices

        delegate: Pressable {
            id: row
            required property var modelData
            readonly property bool chosen: list.current === modelData.value

            Layout.fillWidth: true
            theme: list.theme
            text: modelData.label
            enabled: modelData.value !== -2

            Accessible.role: Accessible.RadioButton
            Accessible.checkable: true
            Accessible.checked: row.chosen

            onClicked: list.picked(row.modelData.value)

            contentItem: RowLayout {
                spacing: list.theme.gapSnug

                RadioMark {
                    theme: list.theme
                    chosen: row.chosen
                }

                LongLabel {
                    theme: list.theme
                    Layout.fillWidth: true
                    text: row.modelData.label
                    font.pixelSize: list.theme.textBody
                    font.bold: row.chosen
                    color: row.enabled ? list.theme.primaryText : list.theme.mutedText
                }
            }
        }
    }
}
