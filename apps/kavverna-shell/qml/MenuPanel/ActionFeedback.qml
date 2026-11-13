import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../Shared"

/// What became of an End, said in the row it was asked from. A signal can be ignored, so
/// "still running" is not a failure but a second question, and it carries the stronger answer
/// with it.
RowLayout {
    id: said

    required property var theme
    /// "working", "ended", "still_running", "refused" or "failed".
    required property string kind
    required property string text

    signal forceEnd()

    spacing: said.theme.gapTight

    LongLabel {
        theme: said.theme
        Layout.fillWidth: true
        horizontalAlignment: Text.AlignRight
        text: said.kind === "still_running" ? "Still running  ·"
            : said.kind === "ended" ? "Ended"
                                   : said.text
        font.pixelSize: said.theme.textSmall
        color: said.kind === "refused" || said.kind === "failed" ? said.theme.ember
             : said.kind === "working" ? said.theme.mutedText
                                       : said.theme.secondaryText
    }

    PillButton {
        theme: said.theme
        visible: said.kind === "still_running"
        compact: true
        text: "Force end"
        onClicked: said.forceEnd()
    }
}
