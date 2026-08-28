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

    KavvernaPanel {
        id: hub
        Component.onCompleted: attach()
    }

    MixerView {
        id: mixer
        Component.onCompleted: attach()
    }

    property int page: 0

    component SectionLabel: Label {
        font.pixelSize: 10
        font.bold: true
        font.letterSpacing: 1.2
        color: root.secondaryText
    }

    component Card: Rectangle {
        radius: 10
        color: root.raised
        Layout.fillWidth: true
    }

    component ChoiceRow: ColumnLayout {
        id: choiceRow
        property alias title: choiceTitle.text
        property alias detail: choiceDetail.text
        property var choices: []
        property int current: 0
        signal picked(int value)

        Layout.fillWidth: true
        spacing: 6

        Label {
            id: choiceTitle
            font.pixelSize: 13
            font.bold: true
            color: root.primaryText
        }

        Label {
            id: choiceDetail
            font.pixelSize: 11
            color: root.secondaryText
            wrapMode: Text.WordWrap
            Layout.fillWidth: true
        }

        RowLayout {
            Layout.fillWidth: true
            spacing: 4

            Repeater {
                model: choiceRow.choices

                delegate: Button {
                    required property var modelData
                    readonly property bool active: modelData.value === choiceRow.current

                    Layout.fillWidth: true
                    implicitHeight: 26
                    text: modelData.label
                    font.pixelSize: 11
                    leftPadding: 2
                    rightPadding: 2
                    onClicked: choiceRow.picked(modelData.value)

                    background: Rectangle {
                        radius: 6
                        color: parent.active ? Qt.rgba(0.24, 0.68, 0.91, 0.30)
                                             : Qt.rgba(1, 1, 1, 0.08)
                    }
                }
            }
        }
    }

    component SettingRow: RowLayout {
        id: settingRow
        property alias title: titleLabel.text
        property alias detail: detailLabel.text
        property bool checked: false
        signal toggled(bool value)

        Layout.fillWidth: true
        spacing: 10

        ColumnLayout {
            Layout.fillWidth: true
            spacing: 1

            Label {
                id: titleLabel
                font.pixelSize: 13
                font.bold: true
                color: root.primaryText
            }

            Label {
                id: detailLabel
                font.pixelSize: 11
                color: root.secondaryText
                wrapMode: Text.WordWrap
                Layout.fillWidth: true
            }
        }

        Switch {
            checked: settingRow.checked
            onToggled: settingRow.toggled(checked)
        }
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
                        color: hub.awake ? root.awakeTint : root.accent
                    }
                }

                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: 4

                    Label {
                        text: hub.showing_settings ? "Settings" : "Kavverna"
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
                visible: !hub.showing_settings
                color: Qt.rgba(1, 1, 1, 0.05)

                RowLayout {
                    anchors.fill: parent
                    anchors.margins: 4
                    spacing: 2

                    Repeater {
                        model: [
                            { glyph: "♪", name: "Sound", page: 1, ready: mixer.available },
                            { glyph: "◴", name: "Monitoring", page: 2, ready: false },
                            { glyph: "✄", name: "Clipboard", page: 3, ready: false },
                            { glyph: "⚡", name: "Energy", page: 0, ready: true }
                        ]

                        delegate: Rectangle {
                            required property var modelData
                            readonly property bool current: root.page === modelData.page

                            Layout.fillWidth: true
                            Layout.fillHeight: true
                            radius: 8
                            color: current ? Qt.rgba(0.24, 0.68, 0.91, 0.22) : "transparent"

                            Label {
                                anchors.centerIn: parent
                                text: parent.modelData.glyph
                                font.pixelSize: 16
                                color: parent.current ? root.accent
                                     : parent.modelData.ready ? Qt.rgba(1, 1, 1, 0.6)
                                                              : Qt.rgba(1, 1, 1, 0.25)
                            }

                            ToolTip.visible: hover.hovered
                            ToolTip.text: modelData.ready ? modelData.name
                                                          : modelData.name + " is not built yet"

                            HoverHandler { id: hover }
                            TapHandler {
                                enabled: modelData.ready
                                onTapped: root.page = modelData.page
                            }
                        }
                    }
                }
            }

            ColumnLayout {
                Layout.fillWidth: true
                visible: !hub.showing_settings && root.page === 0
                spacing: 12

                SectionLabel { text: "ENERGY" }

                Card {
                    implicitHeight: awakeCard.implicitHeight + 24

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
                                    text: hub.awake ? hub.awake_summary
                                                    : "May suspend when idle"
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
                                    { label: "4h", minutes: 240 },
                                    { label: "8h", minutes: 480 }
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

                        RowLayout {
                            Layout.fillWidth: true
                            visible: hub.timed
                            spacing: 6

                            Label {
                                text: "Add more"
                                font.pixelSize: 11
                                color: root.secondaryText
                            }

                            Repeater {
                                model: [
                                    { label: "+15m", minutes: 15 },
                                    { label: "+30m", minutes: 30 },
                                    { label: "+1h", minutes: 60 }
                                ]

                                delegate: Button {
                                    required property var modelData

                                    Layout.fillWidth: true
                                    implicitHeight: 24
                                    text: modelData.label
                                    font.pixelSize: 11
                                    leftPadding: 2
                                    rightPadding: 2
                                    onClicked: hub.extend_minutes(modelData.minutes)

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
                            onToggled: hub.choose_display_sleep(checked)
                        }
                    }
                }
            }

            ColumnLayout {
                Layout.fillWidth: true
                visible: !hub.showing_settings && root.page === 1
                spacing: 12

                SectionLabel { text: "OUTPUT" }

                Card {
                    implicitHeight: outputCard.implicitHeight + 24

                    ColumnLayout {
                        id: outputCard
                        anchors.fill: parent
                        anchors.margins: 12
                        spacing: 8

                        Repeater {
                            model: mixer.output_names.length

                            delegate: ColumnLayout {
                                required property int index
                                readonly property bool isDefault:
                                    mixer.output_ids[index] === mixer.default_output_id

                                Layout.fillWidth: true
                                spacing: 2

                                RowLayout {
                                    Layout.fillWidth: true
                                    spacing: 6

                                    Label {
                                        text: parent.parent.isDefault ? "●" : "○"
                                        font.pixelSize: 10
                                        color: parent.parent.isDefault ? root.accent
                                                                       : root.secondaryText
                                    }

                                    Label {
                                        Layout.fillWidth: true
                                        text: mixer.output_names[parent.parent.index]
                                        font.pixelSize: 11
                                        font.bold: parent.parent.isDefault
                                        color: root.primaryText
                                        elide: Text.ElideRight

                                        TapHandler {
                                            onTapped: mixer.make_default_output(
                                                mixer.output_ids[parent.parent.parent.index])
                                        }
                                    }

                                    Label {
                                        text: mixer.output_volumes[parent.parent.index] + "%"
                                        font.pixelSize: 11
                                        color: root.secondaryText
                                    }
                                }

                                Slider {
                                    Layout.fillWidth: true
                                    implicitHeight: 18
                                    from: 0
                                    to: 100
                                    value: mixer.output_volumes[parent.index]
                                    onMoved: mixer.set_output_volume(
                                        mixer.output_ids[parent.index], Math.round(value))
                                }
                            }
                        }
                    }
                }

                SectionLabel { text: "APPLICATIONS" }

                Card {
                    implicitHeight: streamCard.implicitHeight + 24

                    ColumnLayout {
                        id: streamCard
                        anchors.fill: parent
                        anchors.margins: 12
                        spacing: 8

                        Label {
                            visible: mixer.stream_names.length === 0
                            text: "Nothing is playing"
                            font.pixelSize: 11
                            color: root.secondaryText
                        }

                        Repeater {
                            model: mixer.stream_names.length

                            delegate: ColumnLayout {
                                required property int index

                                Layout.fillWidth: true
                                spacing: 2

                                RowLayout {
                                    Layout.fillWidth: true
                                    spacing: 6

                                    Label {
                                        Layout.fillWidth: true
                                        text: mixer.stream_names[parent.parent.index]
                                        font.pixelSize: 11
                                        color: root.primaryText
                                        elide: Text.ElideRight
                                    }

                                    Label {
                                        text: mixer.stream_volumes[parent.parent.index] + "%"
                                        font.pixelSize: 11
                                        color: mixer.stream_volumes[parent.parent.index] > 100
                                               ? root.awakeTint : root.secondaryText
                                    }
                                }

                                RowLayout {
                                    Layout.fillWidth: true
                                    spacing: 6

                                    Slider {
                                        Layout.fillWidth: true
                                        implicitHeight: 18
                                        from: 0
                                        to: 200
                                        value: mixer.stream_volumes[parent.parent.index]
                                        onMoved: mixer.set_stream_volume(
                                            mixer.stream_ids[parent.parent.index],
                                            Math.round(value))
                                    }

                                    Label {
                                        text: mixer.stream_muted[parent.parent.index] ? "🔇" : "🔊"
                                        font.pixelSize: 12
                                        TapHandler {
                                            onTapped: mixer.mute_stream(
                                                mixer.stream_ids[parent.parent.parent.index],
                                                !mixer.stream_muted[parent.parent.parent.index])
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                SectionLabel { text: "MICROPHONE" }

                Card {
                    implicitHeight: micCard.implicitHeight + 24

                    RowLayout {
                        id: micCard
                        anchors.fill: parent
                        anchors.margins: 12
                        spacing: 10

                        ColumnLayout {
                            Layout.fillWidth: true
                            spacing: 1

                            Label {
                                text: mixer.default_input
                                font.pixelSize: 12
                                font.bold: true
                                color: root.primaryText
                                elide: Text.ElideRight
                                Layout.fillWidth: true
                            }

                            Label {
                                text: mixer.inputs_muted ? "Every microphone is muted"
                                                         : mixer.input_names.length + " inputs"
                                font.pixelSize: 11
                                color: root.secondaryText
                            }
                        }

                        Button {
                            text: mixer.inputs_muted ? "Unmute all" : "Mute all"
                            font.pixelSize: 11
                            implicitHeight: 26
                            onClicked: mixer.mute_every_input(!mixer.inputs_muted)
                        }
                    }
                }
            }

            ColumnLayout {
                Layout.fillWidth: true
                visible: hub.showing_settings
                spacing: 12

                SectionLabel { text: "STARTUP" }

                Card {
                    implicitHeight: startupCard.implicitHeight + 24

                    ColumnLayout {
                        id: startupCard
                        anchors.fill: parent
                        anchors.margins: 12
                        spacing: 12

                        SettingRow {
                            title: "Start with the system"
                            detail: "Adds a desktop entry to the session autostart folder."
                            checked: hub.launch_at_login
                            onToggled: (value) => hub.choose_launch_at_login(value)
                        }

                        SettingRow {
                            title: "Restore keep awake on start"
                            detail: "Hold off sleep again as soon as Kavverna launches."
                            checked: hub.restore_on_start
                            onToggled: (value) => hub.choose_restore_on_start(value)
                        }
                    }
                }

                SectionLabel { text: "ENERGY" }

                Card {
                    implicitHeight: energyCard.implicitHeight + 24

                    ColumnLayout {
                        id: energyCard
                        anchors.fill: parent
                        anchors.margins: 12
                        spacing: 14

                        SettingRow {
                            title: "Let displays sleep"
                            detail: "Blocks automatic suspend only, so screens still turn off and a deliberate suspend still works."
                            checked: hub.allow_display_sleep
                            onToggled: (value) => hub.choose_display_sleep(value)
                        }

                        SettingRow {
                            title: "Middle click toggles"
                            detail: "Middle click the tray icon to switch keep awake on and off. The right button belongs to the menu."
                            checked: hub.middle_click_toggle
                            onToggled: (value) => hub.choose_middle_click_toggle(value)
                        }

                        ChoiceRow {
                            title: "Default duration"
                            detail: "Used by the switch and by auto start."
                            current: hub.default_minutes
                            choices: [
                                { label: "∞", value: 0 },
                                { label: "15m", value: 15 },
                                { label: "30m", value: 30 },
                                { label: "1h", value: 60 },
                                { label: "2h", value: 120 },
                                { label: "4h", value: 240 }
                            ]
                            onPicked: (value) => hub.choose_default_minutes(value)
                        }
                    }
                }

                SectionLabel { text: "MOUSE JIGGLE" }

                Card {
                    implicitHeight: jiggleCard.implicitHeight + 24

                    ColumnLayout {
                        id: jiggleCard
                        anchors.fill: parent
                        anchors.margins: 12
                        spacing: 14

                        SettingRow {
                            title: "Nudge the pointer"
                            detail: hub.jiggle_available
                                    ? "Moves the pointer and puts it back, for applications that watch for input rather than power inhibitions."
                                    : "Needs ydotool and a running ydotoold."
                            checked: hub.mouse_jiggle
                            onToggled: (value) => hub.choose_mouse_jiggle(value)
                        }

                        ChoiceRow {
                            visible: hub.mouse_jiggle
                            title: "Every"
                            current: hub.jiggle_minutes
                            choices: [
                                { label: "1m", value: 1 },
                                { label: "2m", value: 2 },
                                { label: "5m", value: 5 },
                                { label: "10m", value: 10 },
                                { label: "15m", value: 15 }
                            ]
                            onPicked: (value) => hub.choose_jiggle_minutes(value)
                        }
                    }
                }

                Label {
                    Layout.fillWidth: true
                    text: hub.settings_path
                    font.pixelSize: 9
                    color: Qt.rgba(1, 1, 1, 0.3)
                    elide: Text.ElideMiddle
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
                    text: hub.showing_settings ? "‹  Back" : "⚙  Settings"
                    font.pixelSize: 12
                    color: root.secondaryText
                    Layout.fillWidth: true

                    TapHandler { onTapped: hub.show_settings(!hub.showing_settings) }
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
