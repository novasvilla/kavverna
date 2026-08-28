import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.kde.layershell as LayerShell
import dev.kavverna.shell

Window {
    id: root

    readonly property color surface: Qt.rgba(0.11, 0.11, 0.12, 0.97)
    readonly property color raised: Qt.rgba(1, 1, 1, 0.06)
    readonly property color hairline: Qt.rgba(1, 1, 1, 0.09)
    readonly property color primaryText: Qt.rgba(1, 1, 1, 0.95)
    readonly property color secondaryText: Qt.rgba(1, 1, 1, 0.52)
    readonly property color accent: "#3DAEE9"
    readonly property color awakeTint: "#E9B44C"

    width: 360
    // Two nested margins sit between the layout and the window edge: the card's 6 and the
    // content's 12, on both sides.
    height: body.implicitHeight + 36
    visible: hub.panel_open
    color: "transparent"

    onActiveChanged: if (!active && hub.panel_open) hub.close_panel()

    // Anchored rather than positioned: a Wayland client cannot place its own window, so the
    // panel hangs off the screen edge nearest the tray instead.
    LayerShell.Window.anchors: LayerShell.Window.AnchorBottom | LayerShell.Window.AnchorRight
    LayerShell.Window.layer: LayerShell.Window.LayerOverlay
    LayerShell.Window.margins: Qt.rect(0, 0, 12, 12)
    LayerShell.Window.keyboardInteractivity: LayerShell.Window.KeyboardInteractivityOnDemand
    LayerShell.Window.scope: "kavverna-panel"

    KavvernaPanel {
        id: hub
        Component.onCompleted: attach()
    }

    Rectangle {
        anchors.fill: parent
        anchors.margins: 6
        radius: 14
        color: root.surface
        border.width: 1
        border.color: root.hairline

        ColumnLayout {
            id: body
            anchors.fill: parent
            anchors.margins: 12
            spacing: 12

            RowLayout {
                Layout.fillWidth: true
                spacing: 10

                Rectangle {
                    implicitWidth: 38
                    implicitHeight: 38
                    radius: 9
                    color: root.raised

                    Label {
                        anchors.centerIn: parent
                        text: "K"
                        font.pixelSize: 20
                        font.bold: true
                        color: root.accent
                    }
                }

                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: 4

                    Label {
                        text: "Kavverna"
                        font.pixelSize: 16
                        font.bold: true
                        color: root.primaryText
                    }

                    Rectangle {
                        implicitWidth: pill.implicitWidth + 18
                        implicitHeight: 21
                        radius: 10
                        color: hub.awake ? Qt.rgba(0.91, 0.71, 0.30, 0.18)
                                         : Qt.rgba(1, 1, 1, 0.07)

                        RowLayout {
                            id: pill
                            anchors.centerIn: parent
                            spacing: 6

                            Rectangle {
                                implicitWidth: 7
                                implicitHeight: 7
                                radius: 4
                                color: hub.awake ? root.awakeTint : root.secondaryText
                            }

                            Label {
                                text: hub.awake_summary
                                font.pixelSize: 11
                                font.bold: true
                                color: hub.awake ? root.awakeTint : root.secondaryText
                            }
                        }
                    }
                }
            }

            Rectangle {
                Layout.fillWidth: true
                implicitHeight: 42
                radius: 10
                color: Qt.rgba(1, 1, 1, 0.05)

                RowLayout {
                    anchors.fill: parent
                    anchors.margins: 4
                    spacing: 2

                    Repeater {
                        model: [
                            { glyph: "♪", name: "Sound", ready: false },
                            { glyph: "◴", name: "Monitoring", ready: false },
                            { glyph: "✄", name: "Clipboard", ready: false },
                            { glyph: "⚡", name: "Energy", ready: true }
                        ]

                        delegate: Rectangle {
                            required property var modelData

                            Layout.fillWidth: true
                            Layout.fillHeight: true
                            radius: 8
                            color: modelData.ready ? Qt.rgba(0.24, 0.68, 0.91, 0.22) : "transparent"

                            Label {
                                anchors.centerIn: parent
                                text: parent.modelData.glyph
                                font.pixelSize: 16
                                color: parent.modelData.ready ? root.accent
                                                              : Qt.rgba(1, 1, 1, 0.25)
                            }

                            ToolTip.visible: hover.hovered
                            ToolTip.text: modelData.ready ? modelData.name
                                                          : modelData.name + " is not built yet"

                            HoverHandler { id: hover }
                        }
                    }
                }
            }

            Label {
                text: "ENERGY"
                font.pixelSize: 10
                font.bold: true
                font.letterSpacing: 1.2
                color: root.secondaryText
            }

            Rectangle {
                Layout.fillWidth: true
                implicitHeight: awakeCard.implicitHeight + 24
                radius: 10
                color: root.raised

                ColumnLayout {
                    id: awakeCard
                    anchors.fill: parent
                    anchors.margins: 12
                    spacing: 10

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 10

                        Label {
                            text: "⚡"
                            font.pixelSize: 17
                            color: root.awakeTint
                        }

                        ColumnLayout {
                            Layout.fillWidth: true
                            spacing: 1

                            Label {
                                text: "Keep awake"
                                font.pixelSize: 13
                                font.bold: true
                                color: root.primaryText
                            }

                            Label {
                                text: hub.awake ? hub.awake_summary : "May suspend when idle"
                                font.pixelSize: 11
                                color: root.secondaryText
                                elide: Text.ElideRight
                                Layout.fillWidth: true
                            }
                        }

                        Switch {
                            checked: hub.awake
                            onToggled: hub.toggle_awake()
                        }
                    }

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 4

                        Repeater {
                            model: [
                                { label: "15m", minutes: 15 },
                                { label: "30m", minutes: 30 },
                                { label: "1h", minutes: 60 },
                                { label: "2h", minutes: 120 },
                                { label: "4h", minutes: 240 }
                            ]

                            delegate: Button {
                                required property var modelData

                                Layout.fillWidth: true
                                implicitHeight: 26
                                text: modelData.label
                                font.pixelSize: 11
                                leftPadding: 2
                                rightPadding: 2
                                onClicked: hub.keep_awake_minutes(modelData.minutes)

                                background: Rectangle {
                                    radius: 6
                                    color: parent.down ? Qt.rgba(1, 1, 1, 0.16)
                                                       : Qt.rgba(1, 1, 1, 0.08)
                                }
                            }
                        }
                    }

                    CheckBox {
                        text: "Let displays sleep"
                        checked: hub.allow_display_sleep
                        font.pixelSize: 11
                        onToggled: hub.set_display_sleep(checked)
                    }
                }
            }

            Rectangle {
                Layout.fillWidth: true
                implicitHeight: 1
                color: root.hairline
            }

            RowLayout {
                Layout.fillWidth: true

                Label {
                    text: "⚙  Settings"
                    font.pixelSize: 12
                    color: root.secondaryText
                    Layout.fillWidth: true
                }

                Label {
                    text: "⏻  Quit"
                    font.pixelSize: 12
                    color: root.secondaryText

                    TapHandler { onTapped: Qt.quit() }
                }
            }
        }
    }
}
