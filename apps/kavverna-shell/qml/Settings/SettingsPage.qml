import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../Shared"

ColumnLayout {
    id: page

    required property var theme
    required property var hub
    required property var clipboard
    required property var features
    required property var mixer
    required property var shelf
    /// A row for a utility that was removed would offer settings for something not
    /// running, so each one answers for the utility that owns it.
    required property var shows


    Layout.fillWidth: true
    spacing: 12

    SectionLabel {
        theme: page.theme
        text: "PANEL"
    }

    Card {
        theme: page.theme
        spacing: 12

        ChoiceRow {
            theme: page.theme
            title: "Where the panel opens"
            detail: "By the icon follows the tray, wherever its bar lives. Where I left it "
                    + "keeps the spot the panel was dragged to, one per screen."
            choices: [
                { label: "By the tray icon", value: 0 },
                { label: "Where I left it", value: 1 },
                { label: "Bottom right", value: 2 }
            ]
            current: page.hub.placement
            onPicked: (value) => page.hub.choose_placement(value, Window.width, Window.height)
        }

        Label {
            Layout.fillWidth: true
            text: "Drag the panel by its header to move it."
            font.pixelSize: page.theme.textBody
            color: page.theme.secondaryText
        }
    }

    SectionLabel {
        theme: page.theme
        text: "APPEARANCE"
    }

    Card {
        theme: page.theme
        spacing: 12

        ColumnLayout {
            Layout.fillWidth: true
            spacing: page.theme.gapSnug
            visible: page.shows("themes")

            Label {
                text: "Theme"
                font.pixelSize: page.theme.textStrong
                font.bold: true
                color: page.theme.primaryText
            }

            Repeater {
                model: [
                    { id: "torch", label: "Torch",
                      line: "The cavern as it has always been, lit warm." },
                    { id: "tide", label: "Tide",
                      line: "The cave flooded: deep blue water, a cold bright flame." },
                    { id: "ember", label: "Ember",
                      line: "The cave burning down: red heat, clay in daylight." }
                ]

                delegate: RowLayout {
                    id: themeRow
                    required property var modelData
                    readonly property bool current: page.hub.theme_name === modelData.id
                    readonly property var shade:
                        page.theme.palettes[modelData.id][page.theme.dark ? "dark" : "light"]

                    Layout.fillWidth: true
                    spacing: page.theme.gapSnug

                    Label {
                        text: themeRow.current ? "●" : "○"
                        font.pixelSize: page.theme.textSmall
                        color: themeRow.current ? page.theme.accent
                                                : page.theme.secondaryText
                    }

                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 1

                        Label {
                            text: themeRow.modelData.label
                            font.pixelSize: page.theme.textBody
                            font.bold: themeRow.current
                            color: page.theme.primaryText
                        }

                        Label {
                            Layout.fillWidth: true
                            text: themeRow.modelData.line
                            font.pixelSize: page.theme.textSmall
                            color: page.theme.secondaryText
                            wrapMode: Text.WordWrap
                        }
                    }

                    Row {
                        spacing: 4

                        Repeater {
                            model: [themeRow.shade.accent, themeRow.shade.surface,
                                    themeRow.shade.primaryText]

                            delegate: Rectangle {
                                required property var modelData
                                width: 14
                                height: 14
                                radius: 4
                                color: modelData
                                border.width: 1
                                border.color: page.theme.hairline
                            }
                        }
                    }

                    TapHandler {
                        onTapped: page.hub.choose_theme(themeRow.modelData.id)
                    }
                }
            }
        }

        ChoiceRow {
            theme: page.theme
            title: "Appearance"
            detail: "Follow the desktop, or pick a side."
            choices: [
                { label: "Follow", value: 0 },
                { label: "Dark", value: 1 },
                { label: "Light", value: 2 }
            ]
            current: page.hub.appearance
            onPicked: (value) => page.hub.choose_appearance(value)
        }
    }

    FeaturesCard {
        theme: page.theme
        features: page.features
    }

    SectionLabel {
        theme: page.theme
        text: "SOUND"
        visible: page.shows("microphone-tools") || page.shows("output-switcher")
    }

    Card {
        theme: page.theme
        spacing: 14
        visible: page.shows("microphone-tools") || page.shows("output-switcher")

        ColumnLayout {
            Layout.fillWidth: true
            spacing: page.theme.gapSnug
            visible: page.shows("microphone-tools")

            Label {
                text: "Come back to this microphone"
                font.pixelSize: page.theme.textStrong
                font.bold: true
                color: page.theme.primaryText
            }

            Label {
                Layout.fillWidth: true
                text: "Made the default again whenever it is plugged back in. Choosing another one while it is here still works."
                font.pixelSize: page.theme.textBody
                color: page.theme.secondaryText
                wrapMode: Text.WordWrap
            }

            Repeater {
                model: page.mixer.input_names.length

                delegate: Tick {
                    required property int index

                    Layout.fillWidth: true
                    theme: page.theme
                    text: page.mixer.input_names[index]
                    checked: page.mixer.input_preferred[index]
                    onToggled: page.mixer.choose_preferred_input(
                        page.mixer.input_ids[index], checked)
                }
            }

            Label {
                Layout.fillWidth: true
                visible: page.mixer.input_names.length === 0
                text: "No microphone is connected."
                font.pixelSize: page.theme.textBody
                color: page.theme.mutedText
            }
        }

        ColumnLayout {
            Layout.fillWidth: true
            spacing: page.theme.gapSnug
            visible: page.shows("output-switcher")

            Label {
                text: "Move between these outputs"
                font.pixelSize: page.theme.textStrong
                font.bold: true
                color: page.theme.primaryText
            }

            Label {
                Layout.fillWidth: true
                text: "What the shortcut and the tray menu step through. Leave them all ticked to reach every one."
                font.pixelSize: page.theme.textBody
                color: page.theme.secondaryText
                wrapMode: Text.WordWrap
            }

            Repeater {
                model: page.mixer.output_names.length

                delegate: Tick {
                    required property int index

                    Layout.fillWidth: true
                    theme: page.theme
                    text: page.mixer.output_names[index]
                    checked: page.mixer.output_in_cycle[index]
                    onToggled: page.mixer.choose_output_in_cycle(
                        page.mixer.output_ids[index], checked)
                }
            }
        }
    }

    SectionLabel {
        theme: page.theme
        text: "CLIPBOARD"
        visible: page.shows("clipboard-history") || page.shows("clipboard-auto-clear")
                 || page.shows("clean-url")
    }

    Card {
        theme: page.theme
        spacing: 14
        visible: page.shows("clipboard-history") || page.shows("clipboard-auto-clear")
                 || page.shows("clean-url")

        SettingRow {
            theme: page.theme
            visible: page.shows("clipboard-history")
            title: "Save clipboard history"
            detail: "Everything stays on this machine and can be cleared at any time."
            on: page.clipboard.enabled
            onToggled: (value) => page.clipboard.enable(value)
        }

        SettingRow {
            theme: page.theme
            visible: page.shows("clipboard-history")
            title: "Also save copied images and files"
            detail: "Images join the history, and files are remembered by where they are."
            on: page.clipboard.images_and_files
            onToggled: (value) => page.clipboard.choose_images_and_files(value)
        }

        SettingRow {
            theme: page.theme
            visible: page.shows("clipboard-history")
            title: "Skip text that looks sensitive"
            detail: "Leaves out short strings with no spaces that read like passwords, tokens or keys. Anything an application marks as a secret is never read at all."
            on: page.clipboard.skip_sensitive
            onToggled: (value) => page.clipboard.choose_skip_sensitive(value)
        }

        ChoiceRow {
            theme: page.theme
            visible: page.shows("clipboard-history")
            title: "Keep"
            detail: "Pinned entries do not count toward this."
            current: page.clipboard.limit
            choices: [
                { label: "20", value: 20 },
                { label: "50", value: 50 },
                { label: "100", value: 100 },
                { label: "500", value: 500 },
                { label: "\u221e", value: 0 }
            ]
            onPicked: (value) => page.clipboard.choose_limit(value)
        }

        SettingRow {
            theme: page.theme
            visible: page.shows("clean-url")
            title: "Take the tracking out of copied links"
            detail: "Removes campaign and click parameters the moment a link reaches the clipboard, and leaves everything else exactly as it was. Never touches a copy that carries formatting or files."
            on: page.clipboard.clean_links
            onToggled: (value) => page.clipboard.choose_clean_links(value)
        }

        ChoiceRow {
            theme: page.theme
            visible: page.shows("clipboard-auto-clear")
            title: "Empty the clipboard after"
            detail: "Clears what is still pasteable. Saved entries are left alone, and this works with the history switched off."
            current: page.clipboard.clear_after
            choices: [
                { label: "Never", value: 0 },
                { label: "20s", value: 20 },
                { label: "1m", value: 60 },
                { label: "5m", value: 300 },
                { label: "30m", value: 1800 }
            ]
            onPicked: (value) => page.clipboard.choose_clear_after(value)
        }

        SettingRow {
            theme: page.theme
            visible: page.shows("clipboard-auto-clear")
            title: "Empty it when the machine suspends"
            detail: "Announced by logind just before going to sleep."
            on: page.clipboard.clear_on_suspend
            onToggled: (value) => page.clipboard.choose_clear_on_suspend(value)
        }

        SettingRow {
            theme: page.theme
            visible: page.shows("clipboard-auto-clear")
            title: "Empty it when the screen locks"
            detail: "There is no signal for the displays turning off, so that one is not offered rather than approximated."
            on: page.clipboard.clear_on_screen_lock
            onToggled: (value) => page.clipboard.choose_clear_on_screen_lock(value)
        }

        Label {
            Layout.fillWidth: true
            visible: page.clipboard.clear_after > 0
                     || page.clipboard.clear_on_suspend
                     || page.clipboard.clear_on_screen_lock
            text: "Plasma's own clipboard puts the content straight back whenever anything "
                  + "empties it. Until Prevent empty clipboard is turned off in System "
                  + "Settings under Clipboard, none of this has any visible effect."
            font.pixelSize: page.theme.textBody
            color: page.theme.secondaryText
            wrapMode: Text.WordWrap
        }

        ColumnLayout {
            Layout.fillWidth: true
            spacing: 6
            visible: page.clipboard.klipper_waiting > 0 && page.clipboard.available

            Label {
                text: "Take over from Plasma"
                font.pixelSize: page.theme.textStrong
                font.bold: true
                color: page.theme.primaryText
            }

            Label {
                Layout.fillWidth: true
                text: "Plasma's own clipboard has " + page.clipboard.klipper_waiting
                      + " entries saved. They can be adopted here, keeping the times they "
                      + "already had. Nothing is written to Plasma's file. Turn its history "
                      + "off in System Settings under Clipboard so the two stop competing."
                font.pixelSize: page.theme.textBody
                color: page.theme.secondaryText
                wrapMode: Text.WordWrap
            }

            PillButton {
                theme: page.theme
                Layout.fillWidth: true
                text: "Adopt " + page.clipboard.klipper_waiting + " entries"
                onClicked: page.clipboard.adopt_klipper_history()
            }
        }
    }

    SectionLabel {
        theme: page.theme
        text: "SHELF"
        visible: page.shows("shelf")
    }

    Card {
        theme: page.theme
        spacing: 12
        visible: page.shows("shelf")

        SettingRow {
            theme: page.theme
            title: "Keep a strip on the screen edge"
            detail: "A thin landing zone on the right edge. Dragging onto it opens the shelf, and so does clicking it."
            on: page.shelf.edge_strip
            onToggled: (value) => page.shelf.choose_edge_strip(value)
        }

        SettingRow {
            theme: page.theme
            title: "Keep the shelf across restarts"
            detail: "What was shelved is still there next time. Off, the shelf starts empty and its staged copies are removed."
            on: page.shelf.keep_across_restarts
            onToggled: (value) => page.shelf.choose_keep_across_restarts(value)
        }

        SettingRow {
            theme: page.theme
            title: "Remove items after a drop lands"
            detail: "A drag something accepted takes the item off the shelf. A cancelled drag always keeps it."
            on: page.shelf.remove_after_drop
            onToggled: (value) => page.shelf.choose_remove_after_drop(value)
        }
    }

    SectionLabel {
        theme: page.theme
        text: "ENERGY"
        visible: page.shows("keep-awake")
    }

    Card {
        theme: page.theme
        spacing: 14
        visible: page.shows("keep-awake")

        SettingRow {
            theme: page.theme
            title: "Let displays sleep"
            detail: "Blocks automatic suspend only, so screens still turn off and a deliberate suspend still works."
            on: page.hub.allow_display_sleep
            onToggled: (value) => page.hub.choose_display_sleep(value)
        }

        SettingRow {
            theme: page.theme
            title: "Middle click toggles"
            detail: "Middle click the tray icon to switch keep awake on and off. The right button belongs to the menu."
            on: page.hub.middle_click_toggle
            onToggled: (value) => page.hub.choose_middle_click_toggle(value)
        }

        ChoiceRow {
            theme: page.theme
            title: "Default duration"
            detail: "What the switch and the tray menu start when they are not told how long for."
            current: page.hub.default_minutes
            choices: [
                { label: "∞", value: 0 },
                { label: "15m", value: 15 },
                { label: "30m", value: 30 },
                { label: "1h", value: 60 },
                { label: "2h", value: 120 },
                { label: "4h", value: 240 }
            ]
            onPicked: (value) => page.hub.choose_default_minutes(value)
        }
    }

    SectionLabel {
        theme: page.theme
        text: "TOOLS"
        visible: page.shows("mouse-jiggle")
    }

    Card {
        theme: page.theme
        spacing: 12
        visible: page.shows("mouse-jiggle")

        ChoiceRow {
            theme: page.theme
            title: "Nudge no sooner than"
            current: page.hub.jiggle_shortest
            choices: [
                { label: "1m", value: 1 },
                { label: "2m", value: 2 },
                { label: "5m", value: 5 },
                { label: "10m", value: 10 },
                { label: "15m", value: 15 }
            ]
            onPicked: (value) => page.hub.choose_jiggle_shortest(value)
        }

        ChoiceRow {
            theme: page.theme
            title: "And no later than"
            detail: "The wait is drawn afresh between the two, so it does not look like a timer."
            current: page.hub.jiggle_longest
            choices: [
                { label: "2m", value: 2 },
                { label: "5m", value: 5 },
                { label: "10m", value: 10 },
                { label: "15m", value: 15 },
                { label: "30m", value: 30 }
            ]
            onPicked: (value) => page.hub.choose_jiggle_longest(value)
        }

        ChoiceRow {
            theme: page.theme
            title: "What a nudge does"
            current: page.hub.jiggle_activity
            choices: [
                { label: "Pointer", value: 0 },
                { label: "Key", value: 1 },
                { label: "Both", value: 2 }
            ]
            onPicked: (value) => page.hub.choose_jiggle_activity(value)
        }

        ChoiceRow {
            theme: page.theme
            title: "Which key"
            detail: "For the watchers that count keys rather than pointer movement."
            visible: page.hub.jiggle_activity !== 0
            current: page.hub.jiggle_keystroke
            choices: [
                { label: "Shift", value: 0 },
                { label: "Up and down", value: 1 }
            ]
            onPicked: (value) => page.hub.choose_jiggle_keystroke(value)
        }
    }

    SectionLabel {
        theme: page.theme
        text: "STARTUP"
    }

    Card {
        theme: page.theme
        spacing: 12

        SettingRow {
            theme: page.theme
            title: "Start with the system"
            detail: "Adds a desktop entry to the session autostart folder."
            on: page.hub.launch_at_login
            onToggled: (value) => page.hub.choose_launch_at_login(value)
        }

        SettingRow {
            theme: page.theme
            title: "Restore keep awake on start"
            detail: "Puts back the hold that was running when Kavverna last closed, minus the time that passed."
            on: page.hub.restore_on_start
            onToggled: (value) => page.hub.choose_restore_on_start(value)
        }
    }

    SectionLabel {
        theme: page.theme
        text: "ABOUT"
    }

    Card {
        theme: page.theme
        spacing: 8

        RowLayout {
            Layout.fillWidth: true
            spacing: 8

            Label {
                text: "Kavverna"
                font.pixelSize: 14
                font.bold: true
                color: page.theme.primaryText
            }

            Label {
                text: page.hub.version
                font.pixelSize: page.theme.textBody
                color: page.theme.mutedText
            }

            Item { Layout.fillWidth: true }
        }

        Label {
            Layout.fillWidth: true
            text: "One tray icon for the utilities a Linux desktop is missing. Everything "
                  + "runs on this machine, with no account and nothing sent anywhere."
            font.pixelSize: page.theme.textBody
            color: page.theme.secondaryText
            wrapMode: Text.WordWrap
        }

        Label {
            Layout.fillWidth: true
            text: "Inspired by Vorssaint for macOS. Written from scratch in Rust, not "
                  + "ported from it."
            font.pixelSize: page.theme.textBody
            color: page.theme.secondaryText
            wrapMode: Text.WordWrap
        }

        RowLayout {
            Layout.fillWidth: true
            spacing: 6

            Repeater {
                model: [
                    { label: "Repository", url: "https://github.com/novasvilla/kavverna" },
                    { label: "Issues",
                      url: "https://github.com/novasvilla/kavverna/issues" },
                    { label: "Author", url: "https://github.com/novasvilla" }
                ]

                delegate: PillButton {
                    required property var modelData
                    theme: page.theme
                    Layout.fillWidth: true
                    text: modelData.label
                    onClicked: Qt.openUrlExternally(modelData.url)
                }
            }
        }

        Label {
            Layout.fillWidth: true
            text: "GPL-3.0-or-later. Settings at " + page.hub.settings_path
            font.pixelSize: page.theme.textSmall
            color: page.theme.mutedText
            wrapMode: Text.WrapAnywhere
        }
    }
}
