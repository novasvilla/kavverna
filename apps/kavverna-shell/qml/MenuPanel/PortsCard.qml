pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import "../Shared"

/// What the user's applications are listening on. Closed until asked for, since reading every
/// socket is a scan rather than a reading, and read again each time it opens.
ColumnLayout {
    id: ports

    required property var theme
    /// The PortsView.
    required property var view
    property bool open: false

    Layout.fillWidth: true
    spacing: ports.theme.pad

    onOpenChanged: if (open) {
        ports.view.refresh()
        unfold.restart()
    }
    // Left open, it is read again each time the page comes back rather than showing a list
    // from whenever it was last looked at.
    onVisibleChanged: if (visible && open) ports.view.refresh()

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

    // Laid out like the section labels above it rather than as a settings group, so its title
    // starts where theirs do; the wash reaches past the text instead of moving it.
    Pressable {
        id: heading
        theme: ports.theme
        Layout.fillWidth: true
        text: "Listening ports"
        leftInset: -ports.theme.gapTight
        rightInset: -ports.theme.gapTight
        fill: heading.hovered || heading.down ? ports.theme.control : "transparent"
        onClicked: ports.open = !ports.open

        Accessible.expandable: true
        Accessible.expanded: ports.open

        contentItem: RowLayout {
            spacing: ports.theme.gapSnug

            // Counted only once something has been read: before the first look, nought would
            // claim that nothing listens.
            SectionLabel {
                theme: ports.theme
                Layout.fillWidth: true
                text: "LISTENING PORTS" + (ports.view.count > 0 ? "  ·  " + ports.view.count : "")
                color: ports.open ? ports.theme.accent : ports.theme.secondaryText
            }

            Kirigami.Icon {
                source: "go-next-symbolic"
                implicitWidth: ports.theme.iconSmall
                implicitHeight: ports.theme.iconSmall
                isMask: true
                color: ports.open ? ports.theme.accent : ports.theme.secondaryText
                rotation: ports.open ? 90 : 0

                Behavior on rotation {
                    NumberAnimation { duration: ports.theme.motionQuick; easing.type: Easing.OutCubic }
                }
            }
        }
    }

    Card {
        id: card
        theme: ports.theme
        visible: ports.open
        spacing: ports.theme.gapTight

        RowLayout {
            Layout.fillWidth: true
            spacing: ports.theme.gapSnug

            Label {
                Layout.fillWidth: true
                text: ports.view.status_text.length > 0 ? ports.view.status_text
                    : ports.view.count === 1 ? "1 port open"
                                             : ports.view.count + " ports open"
                font.pixelSize: ports.theme.textSmall
                color: ports.theme.secondaryText
                elide: Text.ElideRight
            }

            IconButton {
                theme: ports.theme
                source: "view-refresh-symbolic"
                size: ports.theme.iconSmall
                text: "Look again"
                enabled: !ports.view.refreshing
                onClicked: ports.view.refresh()
            }
        }

        Repeater {
            model: ports.open ? ports.view : null

            delegate: Item {
                id: row

                required property string key
                required property int port
                required property string transport
                required property string address
                required property string exposure
                required property string exposure_text
                required property string name
                required property string icon
                required property string url
                required property bool endable
                required property string confirm_text
                required property bool open
                required property string feedback
                required property string feedback_kind

                property bool confirming: false

                Layout.fillWidth: true
                implicitHeight: lines.implicitHeight + 2 * ports.theme.gapTight

                function confirm(asking) {
                    const fromKeyboard = end.visualFocus || cancel.visualFocus || go.visualFocus
                    row.confirming = asking
                    if (fromKeyboard) {
                        (asking ? cancel : end).forceActiveFocus(Qt.TabFocusReason)
                    }
                }

                HoverHandler { id: rowHover }

                Rectangle {
                    x: -ports.theme.gapTight
                    width: parent.width + 2 * ports.theme.gapTight
                    height: parent.height
                    radius: ports.theme.radiusSmall
                    color: row.confirming ? ports.theme.selected
                         : rowHover.hovered ? ports.theme.sunken
                                            : "transparent"
                }

                ColumnLayout {
                    id: lines
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.verticalCenter: parent.verticalCenter
                    spacing: 2

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: ports.theme.gapSnug
                        opacity: row.open ? 1 : ports.theme.gone

                        Label {
                            text: row.port
                            font.family: "monospace"
                            font.pixelSize: ports.theme.textBody
                            font.bold: true
                            color: ports.theme.primaryText
                        }

                        Rectangle {
                            implicitWidth: protocol.implicitWidth + 2 * ports.theme.gapTight
                            implicitHeight: protocol.implicitHeight + 2
                            radius: ports.theme.gapTight
                            color: ports.theme.control

                            Label {
                                id: protocol
                                anchors.centerIn: parent
                                text: row.transport
                                font.pixelSize: ports.theme.textFine
                                font.bold: true
                                color: ports.theme.secondaryText
                            }
                        }

                        // Reachable from other machines is the one exposure worth a second look.
                        LongLabel {
                            theme: ports.theme
                            Layout.fillWidth: true
                            text: row.exposure_text
                            font.pixelSize: ports.theme.textSmall
                            color: row.exposure === "everywhere" ? ports.theme.warm
                                                                 : ports.theme.secondaryText
                        }

                        // Revealed on hover or Tab, but always in the layout, so the rows keep
                        // their shape and the keyboard can reach a button it cannot yet see.
                        RowLayout {
                            spacing: 0
                            opacity: rowHover.hovered || browse.visualFocus || copyPort.visualFocus
                                     || end.visualFocus || row.confirming ? 1 : 0

                            Behavior on opacity {
                                NumberAnimation { duration: ports.theme.motionQuick }
                            }

                            IconButton {
                                id: browse
                                theme: ports.theme
                                source: "globe-symbolic"
                                size: ports.theme.iconSmall
                                text: "Open in browser"
                                // Kept as a gap where there is nothing to open, so the other
                                // two stay in the same place on every row.
                                enabled: row.url.length > 0 && row.open
                                opacity: row.url.length > 0 ? 1 : 0
                                onClicked: Qt.openUrlExternally(row.url)
                            }

                            IconButton {
                                id: copyPort
                                theme: ports.theme
                                source: copied.running ? "checkmark-symbolic" : "edit-copy-symbolic"
                                size: ports.theme.iconSmall
                                text: copied.running ? "Copied" : "Copy port"
                                onClicked: {
                                    ports.copy(String(row.port))
                                    copied.restart()
                                }

                                Timer {
                                    id: copied
                                    interval: 1200
                                }
                            }

                            IconButton {
                                id: end
                                theme: ports.theme
                                source: "process-stop-symbolic"
                                size: ports.theme.iconSmall
                                text: row.endable ? "End" : "Started by the system, not by an app of yours"
                                enabled: row.endable && row.open && !row.confirming
                                onClicked: row.confirm(true)
                            }
                        }
                    }

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: ports.theme.gapSnug

                        Kirigami.Icon {
                            source: row.icon.length > 0 ? row.icon : "application-x-executable"
                            // A name the icon theme lacks gets the same icon as no name at all.
                            fallback: "application-x-executable"
                            implicitWidth: ports.theme.iconMedium
                            implicitHeight: ports.theme.iconMedium
                            opacity: row.open ? 1 : ports.theme.gone
                        }

                        LongLabel {
                            theme: ports.theme
                            Layout.fillWidth: true
                            text: row.name
                            elide: Text.ElideMiddle
                            font.pixelSize: ports.theme.textBody
                            color: ports.theme.primaryText
                            opacity: row.open ? 1 : ports.theme.gone
                        }

                        Label {
                            visible: row.feedback_kind.length === 0
                            text: row.address
                            font.family: "monospace"
                            font.pixelSize: ports.theme.textFine
                            color: ports.theme.mutedText
                            opacity: row.open ? 1 : ports.theme.gone
                        }

                        ActionFeedback {
                            theme: ports.theme
                            Layout.maximumWidth: row.width * 0.6
                            visible: row.feedback_kind.length > 0
                            kind: row.feedback_kind
                            text: row.feedback
                            onForceEnd: ports.view.force_end_port(row.key)
                        }
                    }

                    Label {
                        Layout.fillWidth: true
                        Layout.topMargin: ports.theme.gapTight
                        visible: row.confirming
                        text: row.confirm_text + "?"
                        font.pixelSize: ports.theme.textSmall
                        color: ports.theme.primaryText
                        wrapMode: Text.WordWrap
                    }

                    RowLayout {
                        Layout.fillWidth: true
                        visible: row.confirming
                        spacing: ports.theme.gapTight

                        Item { Layout.fillWidth: true }

                        PillButton {
                            id: go
                            theme: ports.theme
                            compact: true
                            text: "End"
                            fill: go.down ? ports.theme.controlDown : ports.theme.emberGlow
                            onClicked: {
                                ports.view.end_port(row.key)
                                row.confirm(false)
                            }
                        }

                        PillButton {
                            id: cancel
                            theme: ports.theme
                            compact: true
                            text: "Cancel"
                            onClicked: row.confirm(false)
                        }
                    }
                }
            }
        }
    }

    // A fade rather than a growing height, for the same reason as a settings group: the
    // window follows its content, and a height animated frame by frame resizes it every frame.
    NumberAnimation {
        id: unfold
        target: card
        property: "opacity"
        from: 0
        to: 1
        duration: ports.theme.motionCalm
        easing.type: Easing.OutCubic
    }
}
