import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import "../Shared"

/// The label line of a meter, and the question it can answer: pressed, the applications
/// spending that resource unfold under it. A meter with nothing to unfold has no chevron and
/// no room kept for one, so its label lines up with the plain rows of the card.
Pressable {
    id: heading

    property string label
    property string value
    /// Whether a fold hangs under this line at all.
    property bool foldable: true
    property bool open: false

    Layout.fillWidth: true
    enabled: heading.foldable
    text: heading.label
    // The wash reaches a little past the text, so hovering does not paint right up to a letter.
    leftInset: -heading.theme.gapTight
    rightInset: -heading.theme.gapTight
    fill: heading.foldable && (heading.hovered || heading.down) ? heading.theme.control
                                                                : "transparent"

    Accessible.expandable: heading.foldable
    Accessible.expanded: heading.open

    contentItem: RowLayout {
        spacing: heading.theme.gapSnug

        Kirigami.Icon {
            source: "go-next-symbolic"
            implicitWidth: heading.theme.iconSmall
            implicitHeight: heading.theme.iconSmall
            isMask: true
            color: heading.open ? heading.theme.accent : heading.theme.mutedText
            visible: heading.foldable
            rotation: heading.open ? 90 : 0

            Behavior on rotation {
                NumberAnimation { duration: heading.theme.motionQuick; easing.type: Easing.OutCubic }
            }
        }

        // The label carries live readings beside its name, a temperature and a power draw
        // for the GPU, and gives way to the value on its right rather than pushing it out.
        LongLabel {
            theme: heading.theme
            Layout.fillWidth: true
            text: heading.label
            font.pixelSize: heading.theme.textBody
            color: heading.open ? heading.theme.primaryText : heading.theme.secondaryText
        }

        Label {
            visible: heading.value.length > 0
            text: heading.value
            font.pixelSize: heading.theme.textBody
            font.bold: true
            color: heading.theme.primaryText
        }
    }
}
