import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.kde.layershell as LayerShell
import dev.kavverna.shell
import "MenuPanel"
import "Settings"

Window {
    id: root

    readonly property int energyPage: 0
    readonly property int soundPage: 1
    readonly property int monitoringPage: 2
    readonly property int clipboardPage: 3
    readonly property int toolsPage: 4
    property int page: hub.page

    width: 360
    // A panel the length of the screen is one nobody reads, so every page keeps to the height
    // the rest of them use.
    readonly property int tallest: Math.min(720, Screen.desktopAvailableHeight - 24)
    // Two nested margins sit between the layout and the window edge, the card's 6 and the
    // content's 12, and each is paid on both sides.
    height: Math.min(body.implicitHeight + 36, tallest)
    visible: hub.panel_open
    color: "transparent"

    // Anchored rather than positioned: a Wayland client cannot place its own window, so the
    // panel hangs off the screen edge nearest the tray instead.
    LayerShell.Window.anchors: LayerShell.Window.AnchorBottom | LayerShell.Window.AnchorRight
    LayerShell.Window.layer: LayerShell.Window.LayerOverlay
    LayerShell.Window.margins: Qt.rect(0, 0, 12, 12)
    LayerShell.Window.keyboardInteractivity: LayerShell.Window.KeyboardInteractivityOnDemand
    LayerShell.Window.scope: "kavverna-panel"
    // Pinned to the window's own screen, which is the primary one, rather than letting the
    // compositor pick: the active screen follows focus and would drag the panel into a
    // fullscreen game on the other monitor.
    LayerShell.Window.screenConfiguration: LayerShell.Window.ScreenFromQWindow

    // Closing on lost focus reads well until anything else takes focus on its own: a
    // fullscreen window reclaiming it would shut the panel a moment after it opened. The
    // tray icon and Escape close it instead, which is predictable.
    Shortcut {
        sequence: "Escape"
        onActivated: hub.dismiss()
    }

    // 0 follows the desktop, 1 is the cavern, 2 is its mouth.
    Theme {
        id: theme
        dark: hub.appearance === 0 ? Application.styleHints.colorScheme !== Qt.Light
                                   : hub.appearance === 1
    }

    KavvernaPanel {
        id: hub
        Component.onCompleted: {
            attach()
            report_screen(Screen.desktopAvailableWidth, Screen.desktopAvailableHeight)
        }
    }

    MixerView {
        id: mixer
        Component.onCompleted: attach()
    }

    VitalsView {
        id: vitals
        Component.onCompleted: attach()
    }

    ClipboardView {
        id: clipboard
        Component.onCompleted: attach()
    }

    Rectangle {
        anchors.fill: parent
        anchors.margins: 6
        radius: 14
        color: theme.surface
        border.width: 1
        border.color: theme.hairline

        ColumnLayout {
            id: body
            anchors.fill: parent
            anchors.margins: theme.pad
            spacing: 12

            RowLayout {
                Layout.fillWidth: true
                spacing: 10

                Rectangle {
                    implicitWidth: 38
                    implicitHeight: 38
                    radius: 9
                    color: theme.raised

                    Label {
                        anchors.centerIn: parent
                        text: "K"
                        font.pixelSize: 20
                        font.bold: true
                        color: hub.awake ? theme.accent : theme.secondaryText
                    }
                }

                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: 4

                    Label {
                        text: hub.showing_settings ? "Settings" : "Kavverna"
                        font.pixelSize: theme.textTitle
                        font.bold: true
                        color: theme.primaryText
                    }

                    Rectangle {
                        implicitWidth: pill.implicitWidth + 18
                        implicitHeight: 21
                        radius: 10
                        color: hub.awake ? Qt.alpha(theme.accent, 0.18) : theme.sunken

                        RowLayout {
                            id: pill
                            anchors.centerIn: parent
                            spacing: 6

                            Rectangle {
                                implicitWidth: 7
                                implicitHeight: 7
                                radius: 4
                                color: hub.awake ? theme.warm : theme.secondaryText
                            }

                            Label {
                                text: hub.awake_summary
                                font.pixelSize: theme.textBody
                                font.bold: true
                                color: hub.awake ? theme.warm : theme.secondaryText
                            }
                        }
                    }
                }
            }

            Rectangle {
                Layout.fillWidth: true
                implicitHeight: 42
                radius: 10
                visible: !hub.showing_settings
                color: theme.sunken

                RowLayout {
                    anchors.fill: parent
                    anchors.margins: 4
                    spacing: 2

                    Repeater {
                        model: [
                            { glyph: "\u266a", name: "Sound", page: root.soundPage,
                              ready: mixer.available,
                              unready: "Sound is waiting for PipeWire" },
                            { glyph: "\u25f4", name: "Monitoring", page: root.monitoringPage,
                              ready: true, unready: "" },
                            { glyph: "\u2704", name: "Clipboard", page: root.clipboardPage,
                              ready: true, unready: "" },
                            { glyph: "\ud83d\udee0", name: "Tools", page: root.toolsPage,
                              ready: true, unready: "" },
                            { glyph: "\u26a1", name: "Energy", page: root.energyPage,
                              ready: true, unready: "" }
                        ]

                        delegate: Rectangle {
                            required property var modelData
                            readonly property bool current: root.page === modelData.page

                            Layout.fillWidth: true
                            Layout.fillHeight: true
                            radius: 8
                            color: current ? theme.selected : "transparent"

                            Label {
                                anchors.centerIn: parent
                                text: parent.modelData.glyph
                                font.pixelSize: theme.textTitle
                                color: parent.current ? theme.accent
                                     : parent.modelData.ready ? theme.secondaryText
                                                              : theme.mutedText
                            }

                            ToolTip.visible: hover.hovered
                            ToolTip.text: modelData.ready ? modelData.name : modelData.unready

                            HoverHandler { id: hover }
                            TapHandler {
                                enabled: modelData.ready
                                onTapped: hub.page = modelData.page
                            }
                        }
                    }
                }
            }

            // Only the pages scroll; the header, tabs and footer stay put.
            ScrollView {
                id: scroller
                Layout.fillWidth: true
                Layout.fillHeight: true
                implicitHeight: pages.implicitHeight
                contentWidth: availableWidth
                clip: true
                // Nothing here is ever meant to be wider than the panel, so a horizontal bar
                // would only ever mean a page had overflowed.
                ScrollBar.horizontal.policy: ScrollBar.AlwaysOff

                ColumnLayout {
                    id: pages
                    // The scrollbar takes width off the viewport when it appears. Binding to
                    // the flickable's own width instead leaves the content wider than what is
                    // visible, and the right hand side of every row goes off the edge.
                    width: scroller.availableWidth
                    spacing: 12

                EnergySection {
                    theme: theme
                    hub: hub
                    visible: !hub.showing_settings && root.page === root.energyPage
                }

                SoundSection {
                    theme: theme
                    mixer: mixer
                    visible: !hub.showing_settings && root.page === root.soundPage
                }

                MonitoringSection {
                    theme: theme
                    vitals: vitals
                    visible: !hub.showing_settings && root.page === root.monitoringPage
                }

                ClipboardSection {
                    theme: theme
                    clipboard: clipboard
                    visible: !hub.showing_settings && root.page === root.clipboardPage
                    // Choosing an entry puts it on the clipboard, so the panel gets out of the way
                    // for the paste that follows.
                    onPicked: hub.dismiss()
                }

                ToolsSection {
                    theme: theme
                    hub: hub
                    visible: !hub.showing_settings && root.page === root.toolsPage
                }

                SettingsPage {
                    theme: theme
                    hub: hub
                    clipboard: clipboard
                    visible: hub.showing_settings
                }
                }
            }

            Rectangle {
                Layout.fillWidth: true
                implicitHeight: 1
                color: theme.hairline
            }

            RowLayout {
                Layout.fillWidth: true

                Label {
                    Layout.fillWidth: true
                    text: hub.showing_settings ? "\u2039  Back" : "\u2699  Settings"
                    font.pixelSize: 12
                    color: theme.secondaryText

                    TapHandler { onTapped: hub.show_settings(!hub.showing_settings) }
                }

                Label {
                    text: "\u23fb  Quit"
                    font.pixelSize: 12
                    color: theme.secondaryText

                    TapHandler { onTapped: Qt.quit() }
                }
            }
        }
    }
}
