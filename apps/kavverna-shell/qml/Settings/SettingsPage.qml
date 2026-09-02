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
    /// One group open at a time, so the page is an index that unfolds in place rather than a
    /// column somebody has to scroll past to reach the last setting.
    property string openSection: ""
    property bool cleanRulesOpen: false

    Layout.fillWidth: true
    spacing: 12

    Section {
        theme: page.theme
        title: "PANEL"
        detail: "Where it opens, whether it floats over everything"
        spacing: 12
        open: page.openSection === "PANEL"
        onToggled: page.openSection = open ? "" : "PANEL"

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

        SettingRow {
            theme: page.theme
            title: "Stay above every window"
            detail: "On, the panel floats over everything, fullscreen included. Off, it sits with ordinary panels, fullscreen applications cover it, and it closes once another window takes the focus."
            on: page.hub.panel_on_top
            onToggled: (value) => page.hub.choose_panel_on_top(value)
        }

        Label {
            Layout.fillWidth: true
            text: "Drag the panel by its header to move it."
            font.pixelSize: page.theme.textBody
            color: page.theme.secondaryText
            wrapMode: Text.WordWrap
        }
    }

    Section {
        theme: page.theme
        title: "APPEARANCE"
        detail: "Theme and light or dark"
        spacing: 12
        open: page.openSection === "APPEARANCE"
        onToggled: page.openSection = open ? "" : "APPEARANCE"

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

                        // An inside joke living on the blue theme; the text stays exactly as
                        // it is. Drawn in the window because a ToolTip popup is unreliable
                        // over a layer surface, and inside this plain Row so the RowLayout
                        // around it does not try to lay the bubble out as a cell.
                        Rectangle {
                            visible: themeRow.modelData.id === "tide" && rowHover.hovered
                            anchors.bottom: parent.top
                            anchors.bottomMargin: 2
                            anchors.right: parent.right
                            width: quip.implicitWidth + 16
                            height: quip.implicitHeight + 10
                            radius: page.theme.radiusSmall
                            color: page.theme.surface
                            border.width: 1
                            border.color: page.theme.hairline

                            Label {
                                id: quip
                                anchors.centerIn: parent
                                text: "Ian P. Mode ;-)"
                                font.pixelSize: page.theme.textSmall
                                color: page.theme.primaryText
                            }
                        }
                    }

                    TapHandler {
                        onTapped: page.hub.choose_theme(themeRow.modelData.id)
                    }

                    HoverHandler { id: rowHover }

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
        open: page.openSection === "UTILITIES"
        onToggled: page.openSection = open ? "" : "UTILITIES"
    }

    Section {
        theme: page.theme
        title: "SOUND"
        detail: "Preferred microphone, the outputs the shortcut steps through"
        spacing: 14
        open: page.openSection === "SOUND"
        onToggled: page.openSection = open ? "" : "SOUND"
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
                    checked: index < page.mixer.input_preferred.length
                             && page.mixer.input_preferred[index]
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
                    checked: index < page.mixer.output_in_cycle.length
                             && page.mixer.output_in_cycle[index]
                    onToggled: page.mixer.choose_output_in_cycle(
                        page.mixer.output_ids[index], checked)
                }
            }
        }
    }

    Section {
        theme: page.theme
        title: "CLIPBOARD"
        detail: "History, what is skipped, emptying, URL rules"
        spacing: 14
        open: page.openSection === "CLIPBOARD"
        onToggled: page.openSection = open ? "" : "CLIPBOARD"
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

        PillButton {
            theme: page.theme
            Layout.fillWidth: true
            visible: page.shows("clean-url")
            text: page.cleanRulesOpen ? "Hide URL rules" : "Edit URL rules…"
            onClicked: page.cleanRulesOpen = !page.cleanRulesOpen
        }

        ColumnLayout {
            Layout.fillWidth: true
            visible: page.shows("clean-url") && page.cleanRulesOpen
            spacing: page.theme.gapSnug

            /// Seventy-odd built-in rules are a wall rather than a list, so the ones nobody
            /// has touched stay out of the way: what is shown is what was switched off, what
            /// was added by hand, and whatever the search finds.
            readonly property var shown: {
                const found = []
                const wanted = ruleSearch.text.trim().toLowerCase()
                for (let at = 0; at < page.clipboard.clean_rule_parameters.length; at += 1) {
                    const name = page.clipboard.clean_rule_parameters[at].toLowerCase()
                    const site = page.clipboard.clean_rule_scopes[at].toLowerCase()
                    const off = !page.clipboard.clean_rule_enabled[at]
                    const mine = page.clipboard.clean_rule_custom[at]
                    if (wanted.length > 0 ? (name.indexOf(wanted) >= 0 || site.indexOf(wanted) >= 0)
                                          : (off || mine)) {
                        found.push(at)
                    }
                }
                return found
            }

            TextField {
                id: ruleSearch
                Layout.fillWidth: true
                placeholderText: "Search " + page.clipboard.clean_rule_parameters.length
                                 + " rules by name or site"
                placeholderTextColor: page.theme.mutedText
                font.pixelSize: page.theme.textBody
                color: page.theme.primaryText
                selectionColor: page.theme.selected
                selectedTextColor: page.theme.primaryText
                background: Rectangle {
                    radius: page.theme.radiusSmall
                    color: page.theme.sunken
                    border.width: ruleSearch.activeFocus ? 1 : 0
                    border.color: page.theme.accent
                }
            }

            Label {
                Layout.fillWidth: true
                text: ruleSearch.text.trim().length > 0
                      ? parent.shown.length + " found"
                      : (parent.shown.length === 0
                         ? "Every rule is on. Search to find one and switch it off."
                         : "Switched off or added by you. Search to reach the rest.")
                font.pixelSize: page.theme.textSmall
                color: page.theme.secondaryText
                wrapMode: Text.WordWrap
            }

            Repeater {
                model: parent.shown

                delegate: RowLayout {
                    id: ruleRow
                    required property int modelData
                    readonly property bool yours:
                        modelData < page.clipboard.clean_rule_custom.length
                        && page.clipboard.clean_rule_custom[modelData]

                    Layout.fillWidth: true
                    spacing: page.theme.gapSnug

                    Tick {
                        Layout.fillWidth: true
                        theme: page.theme
                        text: page.clipboard.clean_rule_scopes[ruleRow.modelData] + " · "
                              + page.clipboard.clean_rule_parameters[ruleRow.modelData]
                        // A click writes the box itself and would drop a plain binding; the
                        // rows move as rules are added and searched, and the box would then
                        // sit beside a different rule.
                        Binding on checked {
                            value: ruleRow.modelData < page.clipboard.clean_rule_enabled.length
                                   && page.clipboard.clean_rule_enabled[ruleRow.modelData]
                        }
                        onToggled: page.clipboard.toggle_clean_rule(ruleRow.modelData, checked)
                    }

                    IconButton {
                        theme: page.theme
                        source: "edit-delete"
                        size: 12
                        visible: ruleRow.yours
                        ToolTip.visible: hovered
                        ToolTip.text: "Remove this rule"
                        onClicked: page.clipboard.remove_clean_rule(ruleRow.modelData)
                    }
                }
            }

            Label {
                Layout.fillWidth: true
                Layout.topMargin: page.theme.gapSnug
                text: "A site's rules reach its subdomains and nobody else's, so switching a "
                      + "name off for one site leaves it alone everywhere else."
                font.pixelSize: page.theme.textFine
                color: page.theme.mutedText
                wrapMode: Text.WordWrap
            }

            RowLayout {
                Layout.fillWidth: true
                spacing: page.theme.gapSnug

                TextField {
                    id: cleanDomain
                    Layout.fillWidth: true
                    placeholderText: "Site, or empty for every site"
                    placeholderTextColor: page.theme.mutedText
                    font.pixelSize: page.theme.textBody
                    color: page.theme.primaryText
                    selectionColor: page.theme.selected
                    selectedTextColor: page.theme.primaryText
                    background: Rectangle {
                        radius: page.theme.radiusSmall
                        color: page.theme.sunken
                        border.width: cleanDomain.activeFocus ? 1 : 0
                        border.color: page.theme.accent
                    }
                }

                TextField {
                    id: cleanParameter
                    Layout.preferredWidth: 110
                    placeholderText: "Parameter"
                    placeholderTextColor: page.theme.mutedText
                    font.pixelSize: page.theme.textBody
                    color: page.theme.primaryText
                    selectionColor: page.theme.selected
                    selectedTextColor: page.theme.primaryText
                    onAccepted: addRule.clicked()
                    background: Rectangle {
                        radius: page.theme.radiusSmall
                        color: page.theme.sunken
                        border.width: cleanParameter.activeFocus ? 1 : 0
                        border.color: page.theme.accent
                    }
                }

                PillButton {
                    id: addRule
                    theme: page.theme
                    text: "Add"
                    onClicked: {
                        if (page.clipboard.add_clean_rule(cleanDomain.text, cleanParameter.text)) {
                            cleanDomain.clear()
                            cleanParameter.clear()
                        }
                    }
                }
            }

            Label {
                Layout.fillWidth: true
                visible: page.clipboard.clean_rule_notice.length > 0
                text: page.clipboard.clean_rule_notice
                font.pixelSize: page.theme.textSmall
                color: page.theme.ember
                wrapMode: Text.WordWrap
            }
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

    Section {
        theme: page.theme
        title: "SHELF"
        detail: "Edge strip, where it hangs, what survives a restart"
        spacing: 12
        open: page.openSection === "SHELF"
        onToggled: page.openSection = open ? "" : "SHELF"
        visible: page.shows("shelf")

        SettingRow {
            theme: page.theme
            title: "Keep a strip on the screen edge"
            detail: "A thin landing zone. Dragging onto it opens the shelf, and so does clicking it."
            on: page.shelf.edge_strip
            onToggled: (value) => page.shelf.choose_edge_strip(value)
        }

        ChoiceRow {
            theme: page.theme
            title: "Which edge"
            detail: "Where the strip lives, and where the shelf hangs until it is dragged somewhere. Drag the shelf by its header to place it exactly."
            choices: [
                { label: "Right", value: 0 },
                { label: "Left", value: 1 }
            ]
            current: page.shelf.strip_on_left ? 1 : 0
            onPicked: (value) => page.shelf.choose_strip_edge(value === 1)
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

    Section {
        theme: page.theme
        title: "ENERGY"
        detail: "How long a hold lasts, what it holds"
        spacing: 14
        open: page.openSection === "ENERGY"
        onToggled: page.openSection = open ? "" : "ENERGY"
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

    Section {
        theme: page.theme
        title: "TOOLS"
        detail: "When the mouse is nudged and with what"
        spacing: 12
        open: page.openSection === "TOOLS"
        onToggled: page.openSection = open ? "" : "TOOLS"
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

    Section {
        theme: page.theme
        title: "STARTUP"
        detail: "Starting with the session, restoring a hold"
        spacing: 12
        open: page.openSection === "STARTUP"
        onToggled: page.openSection = open ? "" : "STARTUP"

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

    Section {
        theme: page.theme
        title: "ABOUT"
        detail: "Version, where the settings live, what this machine offers"
        spacing: 8
        open: page.openSection === "ABOUT"
        onToggled: page.openSection = open ? "" : "ABOUT"

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
