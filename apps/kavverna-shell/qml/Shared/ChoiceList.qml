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

        delegate: RowLayout {
            id: row
            required property var modelData
            readonly property bool chosen: list.current === modelData.value

            Layout.fillWidth: true
            spacing: list.theme.gapSnug

            Label {
                text: row.chosen ? "●" : "○"
                font.pixelSize: list.theme.textSmall
                color: row.chosen ? list.theme.accent : list.theme.secondaryText
            }

            Label {
                Layout.fillWidth: true
                text: row.modelData.label
                font.pixelSize: list.theme.textBody
                font.bold: row.chosen
                color: row.modelData.value === -2 ? list.theme.mutedText
                                                  : list.theme.primaryText
                elide: Text.ElideRight

                TapHandler {
                    enabled: row.modelData.value !== -2
                    onTapped: list.picked(row.modelData.value)
                }
            }
        }
    }
}
