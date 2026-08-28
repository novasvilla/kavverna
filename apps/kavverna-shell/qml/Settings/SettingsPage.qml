import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../Shared"

ColumnLayout {
    id: page

    required property var theme
    required property var hub

    Layout.fillWidth: true
    spacing: 12

    SectionLabel {
        theme: page.theme
        text: "STARTUP"
    }

    Card {
        theme: page.theme
        implicitHeight: startup.implicitHeight + 24

        ColumnLayout {
            id: startup
            anchors.fill: parent
            anchors.margins: 12
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
                detail: "Hold off sleep again as soon as Kavverna launches."
                on: page.hub.restore_on_start
                onToggled: (value) => page.hub.choose_restore_on_start(value)
            }
        }
    }

    SectionLabel {
        theme: page.theme
        text: "ENERGY"
    }

    Card {
        theme: page.theme
        implicitHeight: energy.implicitHeight + 24

        ColumnLayout {
            id: energy
            anchors.fill: parent
            anchors.margins: 12
            spacing: 14

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
                detail: "Used by the switch and by auto start."
                current: page.hub.default_minutes
                choices: [
                    { label: "\u221e", value: 0 },
                    { label: "15m", value: 15 },
                    { label: "30m", value: 30 },
                    { label: "1h", value: 60 },
                    { label: "2h", value: 120 },
                    { label: "4h", value: 240 }
                ]
                onPicked: (value) => page.hub.choose_default_minutes(value)
            }
        }
    }

    Label {
        Layout.fillWidth: true
        text: page.hub.settings_path
        font.pixelSize: 9
        color: page.theme.mutedText
        elide: Text.ElideMiddle
    }
}
