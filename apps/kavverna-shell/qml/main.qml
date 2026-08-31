import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.kde.layershell as LayerShell
import org.kde.kirigami as Kirigami
import dev.kavverna.shell
import "MenuPanel"
import "Shared"
import "Settings"
import "Shelf"

Window {
    id: root

    readonly property int energyPage: 0
    readonly property int soundPage: 1
    readonly property int monitoringPage: 2
    readonly property int clipboardPage: 3
    readonly property int toolsPage: 4
    property int page: hub.page

    /// True while the utility is in the features list. Reading `features.installed` here is what
    /// makes every binding below re-evaluate the moment one is switched off.
    function shows(id) {
        const at = features.ids.indexOf(id)
        return at >= 0 && features.installed[at]
    }

    function showsAny(ids) {
        return ids.some(root.shows)
    }

    /// What each page is there for. Stated once so the tab strip and the page itself cannot
    /// disagree about whether it should exist.
    readonly property var pageNeeds: [
        ["keep-awake"],
        ["volume-mixer", "output-switcher", "microphone-tools"],
        ["system-monitor"],
        ["clipboard-history", "clipboard-auto-clear", "clean-url", "clipboard-transform", "shelf"],
        ["mouse-jiggle"]
    ]

    function pageShown(which) {
        return root.showsAny(root.pageNeeds[which])
    }

    /// Opening onto a page whose utilities were all removed would show an empty panel, so the
    /// first one still here is chosen instead.
    function landOnSomethingVisible() {
        if (root.pageShown(hub.page)) {
            return
        }
        for (let which = 0; which < root.pageNeeds.length; which += 1) {
            if (root.pageShown(which)) {
                hub.page = which
                return
            }
        }
    }

    width: hub.panel_width
    // A panel the length of the screen is one nobody reads, so every page keeps to the height
    // the rest of them use.
    readonly property int tallest: Math.min(720, Screen.desktopAvailableHeight - 24)
    // Two nested margins sit between the layout and the window edge, the card's 6 and the
    // content's 12, and each is paid on both sides.
    height: Math.min(body.implicitHeight + 36, tallest)
    visible: hub.panel_open
    color: "transparent"

    // A Wayland client cannot place an ordinary window, but a layer surface is placed by its
    // anchors and margins, and those are ours. The numbers come from Rust, worked out per
    // open: beside the tray icon, at a remembered spot, or the old bottom right corner.
    LayerShell.Window.anchors: (hub.at_bottom ? LayerShell.Window.AnchorBottom
                                              : LayerShell.Window.AnchorTop)
                             | (hub.at_right ? LayerShell.Window.AnchorRight
                                             : LayerShell.Window.AnchorLeft)
    LayerShell.Window.layer: LayerShell.Window.LayerOverlay
    LayerShell.Window.margins: Qt.rect(hub.margin_left, hub.margin_top,
                                       hub.margin_right, hub.margin_bottom)
    LayerShell.Window.keyboardInteractivity: LayerShell.Window.KeyboardInteractivityOnDemand
    LayerShell.Window.scope: "kavverna-panel"
    // Pinned to the window's own screen rather than letting the compositor pick: the active
    // screen follows focus and would drag the panel into a fullscreen game on the other
    // monitor. Placement names a screen when it knows one, and the window follows it.
    LayerShell.Window.screenConfiguration: LayerShell.Window.ScreenFromQWindow

    // Falls back to the first screen rather than to root.screen: a value that read the
    // property it feeds would be a binding loop.
    function screenNamed(name) {
        const all = Qt.application.screens
        for (let i = 0; i < all.length; i += 1) {
            if (all[i].name === name) {
                return all[i]
            }
        }
        return all[0]
    }

    Binding on screen {
        when: hub.panel_screen.length > 0
        value: root.screenNamed(hub.panel_screen)
    }

    // Closing on lost focus reads well until anything else takes focus on its own: a
    // fullscreen window reclaiming it would shut the panel a moment after it opened. The
    // tray icon and Escape close it instead, which is predictable.
    Shortcut {
        sequence: "Escape"
        onActivated: hub.dismiss()
    }

    // 0 follows the desktop, 1 is the cavern, 2 is its mouth. The palette follows the themes
    // utility: removed, the torch applies and the stored choice waits.
    Theme {
        id: theme
        dark: hub.appearance === 0 ? Application.styleHints.colorScheme !== Qt.Light
                                   : hub.appearance === 1
        name: root.shows("themes") ? hub.theme_name : "torch"
    }

    KavvernaPanel {
        id: hub
        Component.onCompleted: {
            // Screens first: attach() may open the panel at once for a launch argument, and
            // placing it needs to know what is connected.
            const lines = []
            const all = Qt.application.screens
            for (let i = 0; i < all.length; i += 1) {
                const s = all[i]
                lines.push(s.name + "\t" + s.virtualX + "\t" + s.virtualY
                           + "\t" + s.width + "\t" + s.height)
            }
            report_screens(lines.join("\n"))
            report_screen(Screen.desktopAvailableWidth, Screen.desktopAvailableHeight)
            attach()
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

    FeaturesView {
        id: features
        Component.onCompleted: attach()
        onInstalledChanged: root.landOnSomethingVisible()
    }

    ShelfView {
        id: shelfBridge
        Component.onCompleted: attach()
    }

    ShelfWindow {
        id: shelfWindow
        theme: theme
        shelf: shelfBridge
        shows: root.shows
    }

    EdgeStrip {
        theme: theme
        shelf: shelfBridge
        shows: root.shows
        home: shelfWindow
    }

    // The shelf's drag ghost, the same gesture as the panel's below.
    Window {
        width: shelfWindow.width
        height: shelfWindow.height
        visible: shelfBridge.ghost_visible
        color: "transparent"
        transientParent: null
        flags: Qt.FramelessWindowHint | Qt.WindowTransparentForInput

        LayerShell.Window.anchors: LayerShell.Window.AnchorTop | LayerShell.Window.AnchorLeft
        LayerShell.Window.layer: LayerShell.Window.LayerOverlay
        LayerShell.Window.margins: Qt.rect(shelfBridge.ghost_left, shelfBridge.ghost_top, 0, 0)
        LayerShell.Window.keyboardInteractivity: LayerShell.Window.KeyboardInteractivityNone
        LayerShell.Window.scope: "kavverna-shelf-ghost"
        LayerShell.Window.screenConfiguration: LayerShell.Window.ScreenFromQWindow

        Image {
            anchors.fill: parent
            anchors.margins: 6
            source: shelfWindow.shelfShot ? shelfWindow.shelfShot.url : ""
            opacity: 0.85
        }

        Rectangle {
            anchors.fill: parent
            anchors.margins: 6
            radius: 14
            color: shelfWindow.shelfShot ? "transparent" : theme.glow
            border.width: 2
            border.color: theme.accent
        }
    }

    // The outline that follows a header drag. The real panel cannot move under the pointer
    // without poisoning the pointer's own readings, so this ghost does the moving and the
    // panel jumps to it on release. Transparent to input, or it would sit under the cursor
    // and swallow the very drag it is drawing.
    Window {
        width: hub.panel_width
        height: root.height
        visible: hub.ghost_visible
        color: "transparent"
        transientParent: null
        flags: Qt.FramelessWindowHint | Qt.WindowTransparentForInput

        LayerShell.Window.anchors: LayerShell.Window.AnchorTop | LayerShell.Window.AnchorLeft
        LayerShell.Window.layer: LayerShell.Window.LayerOverlay
        LayerShell.Window.margins: Qt.rect(hub.ghost_left, hub.ghost_top, 0, 0)
        LayerShell.Window.keyboardInteractivity: LayerShell.Window.KeyboardInteractivityNone
        LayerShell.Window.scope: "kavverna-panel-ghost"
        LayerShell.Window.screenConfiguration: LayerShell.Window.ScreenFromQWindow

        Image {
            anchors.fill: parent
            anchors.margins: 6
            source: root.ghostShot ? root.ghostShot.url : ""
            opacity: 0.85
        }

        Rectangle {
            anchors.fill: parent
            anchors.margins: 6
            radius: 14
            color: root.ghostShot ? "transparent" : theme.glow
            border.width: 2
            border.color: theme.accent
        }
    }

    // What the ghost outline shows while the panel is dragged: a picture of the panel as it
    // was at the press, so the drag looks like moving the thing rather than an empty frame.
    property var ghostShot: null

    Rectangle {
        id: skin
        anchors.fill: parent
        anchors.margins: 6
        radius: 14
        color: theme.surface
        border.width: 1
        border.color: theme.hairline

        // The header strip is the panel's handle. A MouseArea rather than a DragHandler
        // because the handler never activates on this layer surface under KWin. The panel
        // itself holds still for the whole gesture, so these readings stay clean
        // press-relative offsets; the ghost outline is what follows the hand, and the
        // release places the panel where the ghost is.
        MouseArea {
            id: mover
            anchors.top: parent.top
            anchors.left: parent.left
            anchors.right: parent.right
            height: 56
            property real pressX: 0
            property real pressY: 0

            onPressed: (mouse) => {
                pressX = mouse.x
                pressY = mouse.y
                skin.grabToImage((shot) => root.ghostShot = shot)
                hub.drag_begun(root.width, root.height)
            }
            onPositionChanged: (mouse) => {
                if (!pressed) {
                    return
                }
                hub.drag_preview(Math.round(mouse.x - pressX), Math.round(mouse.y - pressY),
                                 root.width, root.height)
            }
            onReleased: hub.drag_commit(root.width, root.height)
        }

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
                        Layout.alignment: Qt.AlignLeft
                        implicitWidth: pill.implicitWidth + 18
                        implicitHeight: 21
                        radius: theme.radius
                        color: hub.awake ? theme.glow : theme.sunken

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
                radius: theme.radius
                visible: !hub.showing_settings
                color: theme.sunken

                RowLayout {
                    anchors.fill: parent
                    anchors.margins: 4
                    spacing: 2

                    Repeater {
                        model: [
                            { icon: "audio-volume-high", name: "Sound", page: root.soundPage,
                              ready: mixer.available,
                              unready: "Sound is waiting for PipeWire",
                              needs: root.pageNeeds[root.soundPage] },
                            { icon: "office-chart-area", name: "Monitoring", page: root.monitoringPage,
                              ready: true, unready: "",
                              needs: root.pageNeeds[root.monitoringPage] },
                            { icon: "edit-paste", name: "Clipboard", page: root.clipboardPage,
                              ready: true, unready: "",
                              needs: root.pageNeeds[root.clipboardPage] },
                            { icon: "input-mouse", name: "Tools", page: root.toolsPage,
                              ready: true, unready: "",
                              needs: root.pageNeeds[root.toolsPage] },
                            { icon: "", name: "Energy", page: root.energyPage,
                              ready: true, unready: "",
                              needs: root.pageNeeds[root.energyPage] }
                        ]

                        delegate: Rectangle {
                            required property var modelData
                            readonly property bool current: root.page === modelData.page

                            visible: root.showsAny(modelData.needs)
                            Layout.fillWidth: visible
                            Layout.fillHeight: true
                            radius: 8
                            color: current ? theme.selected : "transparent"

                            readonly property color mark: current ? theme.accent
                                : modelData.ready ? theme.secondaryText
                                                  : theme.mutedText

                            Kirigami.Icon {
                                anchors.centerIn: parent
                                visible: parent.modelData.icon !== ""
                                source: parent.modelData.icon
                                implicitWidth: 18
                                implicitHeight: 18
                                isMask: true
                                color: parent.mark
                            }

                            Bolt {
                                anchors.centerIn: parent
                                visible: parent.modelData.icon === ""
                                color: parent.mark
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
                    shows: root.shows
                    hub: hub
                    visible: !hub.showing_settings && root.page === root.energyPage
                             && root.pageShown(root.energyPage)
                }

                SoundSection {
                    theme: theme
                    shows: root.shows
                    mixer: mixer
                    visible: !hub.showing_settings && root.page === root.soundPage
                             && root.pageShown(root.soundPage)
                }

                MonitoringSection {
                    theme: theme
                    shows: root.shows
                    vitals: vitals
                    visible: !hub.showing_settings && root.page === root.monitoringPage
                             && root.pageShown(root.monitoringPage)
                }

                ClipboardSection {
                    theme: theme
                    shows: root.shows
                    clipboard: clipboard
                    shelf: shelfBridge
                    visible: !hub.showing_settings && root.page === root.clipboardPage
                             && root.pageShown(root.clipboardPage)
                    // Choosing an entry puts it on the clipboard, so the panel gets out of the way
                    // for the paste that follows.
                    onPicked: hub.dismiss()
                }

                ToolsSection {
                    theme: theme
                    shows: root.shows
                    hub: hub
                    visible: !hub.showing_settings && root.page === root.toolsPage
                             && root.pageShown(root.toolsPage)
                }

                SettingsPage {
                    theme: theme
                    hub: hub
                    clipboard: clipboard
                    features: features
                    mixer: mixer
                    shelf: shelfBridge
                    shows: root.shows
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
