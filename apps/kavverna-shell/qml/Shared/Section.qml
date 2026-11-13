import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

/// One group of settings, closed until it is asked for. The page is a short index of these,
/// so reaching the last group is a tap rather than a page of scrolling, and only one is ever
/// open, so what is open is always the whole page.
ColumnLayout {
    id: section

    required property var theme
    required property string title
    /// What the group is for, in one line, while it is closed.
    required property string detail
    required property bool open

    signal toggled()

    default property alias content: card.content

    Layout.fillWidth: true
    spacing: section.theme.gapSnug

    Pressable {
        id: heading
        Layout.fillWidth: true
        theme: section.theme
        text: section.title
        padding: section.theme.gapSnug
        fill: section.open ? section.theme.selected
            : heading.hovered ? section.theme.control : "transparent"

        Accessible.description: section.detail
        Accessible.expandable: true
        Accessible.expanded: section.open

        onClicked: section.toggled()

        contentItem: RowLayout {
            spacing: section.theme.gapSnug

            ColumnLayout {
                Layout.fillWidth: true
                spacing: 1

                Label {
                    text: section.title
                    font.pixelSize: section.theme.textSmall
                    font.bold: true
                    font.letterSpacing: 1.2
                    color: section.open ? section.theme.accent : section.theme.secondaryText
                }

                LongLabel {
                    theme: section.theme
                    Layout.fillWidth: true
                    visible: !section.open
                    text: section.detail
                    font.pixelSize: section.theme.textFine
                    color: section.theme.mutedText
                }
            }

            Kirigami.Icon {
                source: "go-next-symbolic"
                implicitWidth: section.theme.iconSmall
                implicitHeight: section.theme.iconSmall
                isMask: true
                color: section.open ? section.theme.accent : section.theme.secondaryText
                rotation: section.open ? 90 : 0

                Behavior on rotation {
                    NumberAnimation { duration: 110; easing.type: Easing.OutCubic }
                }
            }
        }
    }

    Card {
        id: card
        theme: section.theme
        visible: section.open
        spacing: section.theme.pad
    }
}
