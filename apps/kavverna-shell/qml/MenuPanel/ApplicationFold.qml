pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import "../Shared"

/// The applications spending one resource, under the meter that measures it. Every meter has
/// one of these and they share a single model, so only the open one is given it: the others
/// hold no rows at all.
ColumnLayout {
    id: fold

    required property var theme
    /// The ProcessView. Not called `processes`, which is also the name of a row's role.
    required property var view
    /// "cpu", "memory", "gpu" or "disk", as `open_fold` names it.
    required property string resource
    required property string title

    // From the view alone: the backend closes the fold when the panel hides, and a copy of the
    // state kept here would reopen it on the next show with nothing measured.
    readonly property bool open: fold.resource.length > 0
                                 && fold.view.open_fold === fold.resource

    Layout.fillWidth: true
    Layout.leftMargin: fold.theme.foldIndent
    visible: fold.open
    spacing: 2

    onVisibleChanged: if (visible) unfold.restart()

    // While the pointer is over the list the rows keep their places, so the row under it is
    // still the one it was aimed at when the press lands.
    HoverHandler {
        onHoveredChanged: fold.view.hold_order(hovered)
    }

    function copy(text) {
        copier.text = text
        copier.selectAll()
        copier.copy()
    }

    // QML has no other door to the clipboard than a text control.
    TextEdit {
        id: copier
        visible: false
    }

    Label {
        Layout.fillWidth: true
        Layout.bottomMargin: fold.theme.gapTight
        text: fold.title
        font.pixelSize: fold.theme.textSmall
        font.bold: true
        color: fold.theme.secondaryText
        elide: Text.ElideRight
    }

    Repeater {
        model: fold.open ? fold.view : null

        delegate: Item {
            id: row

            required property string key
            required property string name
            required property string icon
            required property string value
            required property string secondary
            required property int pid
            required property bool present
            required property string feedback
            required property string feedback_kind

            /// "" while the row only reads, "actions" with its menu out, "end" or "force"
            /// while it asks whether to go ahead.
            property string asking: ""
            readonly property bool confirming: row.asking === "end" || row.asking === "force"

            Layout.fillWidth: true
            implicitHeight: fold.theme.buttonHeightCompact

            function ask(what) {
                const fromKeyboard = more.visualFocus || row.focusInside()
                row.asking = what
                if (fromKeyboard) {
                    if (row.confirming) {
                        cancel.forceActiveFocus(Qt.TabFocusReason)
                    } else if (what === "actions") {
                        endChoice.forceActiveFocus(Qt.TabFocusReason)
                    } else {
                        more.forceActiveFocus(Qt.TabFocusReason)
                    }
                }
            }

            function focusInside() {
                return endChoice.visualFocus || forceChoice.visualFocus || copyChoice.visualFocus
                    || go.visualFocus || cancel.visualFocus
            }

            HoverHandler { id: rowHover }

            Rectangle {
                x: -fold.theme.gapTight
                width: parent.width + 2 * fold.theme.gapTight
                height: parent.height
                radius: fold.theme.radiusSmall
                color: row.confirming ? fold.theme.selected
                     : row.asking === "actions" ? fold.theme.control
                     : rowHover.hovered ? fold.theme.sunken
                                        : "transparent"
            }

            RowLayout {
                anchors.fill: parent
                spacing: fold.theme.gapSnug

                Kirigami.Icon {
                    source: row.icon.length > 0 ? row.icon : "application-x-executable"
                    // A name the icon theme lacks gets the same icon as no name at all.
                    fallback: "application-x-executable"
                    implicitWidth: fold.theme.iconMedium
                    implicitHeight: fold.theme.iconMedium
                    opacity: row.present ? 1 : fold.theme.gone
                }

                LongLabel {
                    theme: fold.theme
                    Layout.fillWidth: true
                    Layout.maximumWidth: row.asking === "" && (row.feedback_kind === "refused"
                                                               || row.feedback_kind === "failed")
                                         ? row.width * 0.3 : Infinity
                    text: row.confirming ? (row.asking === "force" ? "Force end " : "End ")
                                           + row.name + "?"
                                         : row.name
                    elide: Text.ElideMiddle
                    font.pixelSize: fold.theme.textBody
                    color: fold.theme.primaryText
                    opacity: row.present ? 1 : fold.theme.gone
                }

                // What the row reads, while nothing else is being said about it.
                RowLayout {
                    visible: row.asking === "" && row.feedback_kind.length === 0
                    spacing: fold.theme.gapSnug
                    opacity: row.present ? 1 : fold.theme.gone

                    Label {
                        text: row.value
                        font.pixelSize: fold.theme.textBody
                        font.features: { "tnum": 1 }
                        color: fold.theme.secondaryText
                    }

                    Label {
                        visible: row.secondary.length > 0
                        text: row.secondary
                        font.pixelSize: fold.theme.textSmall
                        font.features: { "tnum": 1 }
                        color: fold.theme.mutedText
                    }
                }

                // A refusal is a sentence and the name is already known, so the name gives way
                // to it; it still keeps enough of itself to say which row was refused.
                ActionFeedback {
                    theme: fold.theme
                    Layout.maximumWidth: row.width * 0.7
                    visible: row.asking === "" && row.feedback_kind.length > 0
                    kind: row.feedback_kind
                    text: row.feedback
                    onForceEnd: fold.view.force_end_application(row.key)
                }

                PillButton {
                    id: endChoice
                    theme: fold.theme
                    visible: row.asking === "actions"
                    compact: true
                    text: "End"
                    onClicked: row.ask("end")
                }

                PillButton {
                    id: forceChoice
                    theme: fold.theme
                    visible: row.asking === "actions"
                    compact: true
                    text: "Force end"
                    onClicked: row.ask("force")
                }

                PillButton {
                    id: copyChoice
                    theme: fold.theme
                    visible: row.asking === "actions" && row.pid > 0
                    compact: true
                    text: copied.running ? "Copied" : "Copy PID"
                    onClicked: {
                        fold.copy(String(row.pid))
                        copied.restart()
                    }

                    Timer {
                        id: copied
                        interval: 1200
                        onTriggered: if (row.asking === "actions") row.ask("")
                    }
                }

                PillButton {
                    id: go
                    theme: fold.theme
                    visible: row.confirming
                    compact: true
                    text: row.asking === "force" ? "Force end" : "End"
                    fill: go.down ? fold.theme.controlDown : fold.theme.emberGlow
                    onClicked: {
                        if (row.asking === "force") {
                            fold.view.force_end_application(row.key)
                        } else {
                            fold.view.end_application(row.key)
                        }
                        row.ask("")
                    }
                }

                PillButton {
                    id: cancel
                    theme: fold.theme
                    visible: row.confirming
                    compact: true
                    text: "Cancel"
                    onClicked: row.ask("")
                }

                // Always in the layout, only its opacity changes, so a row never shifts when
                // the pointer reaches it and Tab can still land on it while it is unseen.
                IconButton {
                    id: more
                    theme: fold.theme
                    visible: !row.confirming
                    source: row.asking === "actions" ? "window-close-symbolic" : "view-more-symbolic"
                    size: fold.theme.iconSmall
                    text: row.asking === "actions" ? "Close" : "Actions for " + row.name
                    opacity: rowHover.hovered || more.visualFocus || row.asking !== "" ? 1 : 0
                    onClicked: row.ask(row.asking === "actions" ? "" : "actions")

                    Behavior on opacity {
                        NumberAnimation { duration: fold.theme.motionQuick }
                    }
                }
            }
        }
    }

    Label {
        Layout.fillWidth: true
        Layout.topMargin: fold.theme.gapTight
        visible: fold.view.phase !== "ready" && fold.view.status_text.length > 0
        text: fold.view.status_text
        font.pixelSize: fold.theme.textSmall
        color: fold.theme.secondaryText
        wrapMode: Text.WordWrap
    }

    Label {
        Layout.fillWidth: true
        Layout.topMargin: fold.theme.gapTight
        visible: fold.view.phase === "ready" && fold.view.summary_text.length > 0
        text: fold.view.summary_text
        font.pixelSize: fold.theme.textSmall
        color: fold.theme.mutedText
        wrapMode: Text.WordWrap
    }

    Pressable {
        id: showMore
        theme: fold.theme
        Layout.topMargin: fold.theme.gapTight
        visible: fold.view.has_more
        text: fold.view.expanded ? "Show fewer" : "Show more"
        leftInset: -fold.theme.gapTight
        rightInset: -fold.theme.gapTight
        fill: showMore.hovered ? fold.theme.control : "transparent"
        onClicked: fold.view.expanded = !fold.view.expanded

        contentItem: Label {
            text: showMore.text
            font.pixelSize: fold.theme.textSmall
            color: fold.theme.accent
        }
    }

    // A fade rather than a growing height: the panel's window is sized to its content, and a
    // height animated frame by frame would resize the layer surface on every one of them.
    NumberAnimation {
        id: unfold
        target: fold
        property: "opacity"
        from: 0
        to: 1
        duration: fold.theme.motionCalm
        easing.type: Easing.OutCubic
    }
}
