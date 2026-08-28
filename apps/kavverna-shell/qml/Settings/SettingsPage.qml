import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../Shared"

ColumnLayout {
    id: page

    required property var theme
    required property var hub
    required property var clipboard

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


    SectionLabel {
        theme: page.theme
        text: "CLIPBOARD"
    }

    Card {
        theme: page.theme
        implicitHeight: saving.implicitHeight + 24

        ColumnLayout {
            id: saving
            anchors.fill: parent
            anchors.margins: 12
            spacing: 14

            SettingRow {
                theme: page.theme
                title: "Save clipboard history"
                detail: "Everything stays on this machine and can be cleared at any time."
                on: page.clipboard.enabled
                onToggled: (value) => page.clipboard.enable(value)
            }

            SettingRow {
                theme: page.theme
                title: "Also save copied images and files"
                detail: "Images join the history, and files are remembered by where they are."
                on: page.clipboard.images_and_files
                onToggled: (value) => page.clipboard.choose_images_and_files(value)
            }

            SettingRow {
                theme: page.theme
                title: "Skip text that looks sensitive"
                detail: "Leaves out short strings with no spaces that read like passwords, tokens or keys. Anything an application marks as a secret is never read at all."
                on: page.clipboard.skip_sensitive
                onToggled: (value) => page.clipboard.choose_skip_sensitive(value)
            }

            ChoiceRow {
                theme: page.theme
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

            ColumnLayout {
                Layout.fillWidth: true
                spacing: 6
                visible: page.clipboard.klipper_waiting > 0 && page.clipboard.available

                Label {
                    text: "Take over from Plasma"
                    font.pixelSize: 13
                    font.bold: true
                    color: page.theme.primaryText
                }

                Label {
                    Layout.fillWidth: true
                    text: "Plasma's own clipboard has " + page.clipboard.klipper_waiting
                          + " entries saved. They can be adopted here, keeping the times they "
                          + "already had. Nothing is written to Plasma's file. Turn its history "
                          + "off in System Settings under Clipboard so the two stop competing."
                    font.pixelSize: 11
                    color: page.theme.secondaryText
                    wrapMode: Text.WordWrap
                }

                Button {
                    Layout.fillWidth: true
                    implicitHeight: 26
                    text: "Adopt " + page.clipboard.klipper_waiting + " entries"
                    font.pixelSize: 11
                    onClicked: page.clipboard.adopt_klipper_history()
                }
            }
        }
    }
}
