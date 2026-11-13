pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../Shared"

/// How much of each coding agent's allowance is spent. Three states that must never be
/// confused: nothing read yet, read and nothing found, and agents found.
ColumnLayout {
    id: usage

    required property var theme
    required property var agents

    Layout.fillWidth: true
    spacing: usage.theme.pad

    // Asks for a fresh reading whenever the card comes into view: on reaching the page and on
    // the panel opening onto it. An item's own visibility ignores its window's, so both are
    // watched; the ages beside each figure are worked out when a reading lands and would
    // otherwise go on saying "just now" from whenever that was.
    readonly property bool inView: usage.visible && usage.Window.visibility !== Window.Hidden
    onInViewChanged: if (usage.inView) usage.agents.refresh()

    SectionLabel {
        theme: usage.theme
        text: "AI AGENTS"
    }

    Card {
        theme: usage.theme
        spacing: usage.theme.gap

        Label {
            Layout.fillWidth: true
            visible: !usage.agents.has_reading
            text: "Reading agent usage…"
            font.pixelSize: usage.theme.textBody
            color: usage.theme.secondaryText
        }

        Label {
            Layout.fillWidth: true
            visible: usage.agents.has_reading && usage.agents.agent_count === 0
            text: usage.agents.empty_text
            font.pixelSize: usage.theme.textBody
            color: usage.theme.secondaryText
            wrapMode: Text.WordWrap
        }

        Repeater {
            model: usage.agents.has_reading ? usage.agents.agent_count : 0

            delegate: ColumnLayout {
                id: agent

                required property int index
                readonly property string kind: usage.agents.agent_kinds[agent.index] ?? ""

                Layout.fillWidth: true
                spacing: usage.theme.gapSnug

                RowLayout {
                    Layout.fillWidth: true
                    spacing: usage.theme.gap

                    LongLabel {
                        theme: usage.theme
                        Layout.fillWidth: true
                        text: usage.agents.agent_titles[agent.index] ?? ""
                        font.pixelSize: usage.theme.textStrong
                        font.bold: true
                        color: usage.theme.primaryText
                    }

                    Label {
                        text: usage.agents.agent_ages[agent.index] ?? ""
                        visible: text.length > 0
                        font.pixelSize: usage.theme.textSmall
                        color: usage.theme.mutedText
                    }
                }

                // Windows arrive flattened in agent order, so each agent picks out its own.
                Repeater {
                    model: agent.kind === "allowance" ? usage.agents.window_agents.length : 0

                    delegate: ColumnLayout {
                        id: reading

                        required property int index
                        readonly property string freshness: usage.agents.window_states[reading.index] ?? ""
                        readonly property bool stale: reading.freshness === "stale"
                        // A window that reset after it was read has an unknown figure now. The
                        // old one would read as current, so it is not shown at all.
                        readonly property bool expired: reading.freshness === "expired"
                        readonly property real used: usage.agents.window_used[reading.index] ?? 0

                        visible: usage.agents.window_agents[reading.index] === agent.index
                        Layout.fillWidth: true
                        spacing: 3

                        RowLayout {
                            Layout.fillWidth: true
                            opacity: reading.stale ? 0.6 : 1

                            Label {
                                Layout.fillWidth: true
                                text: usage.agents.window_labels[reading.index] ?? ""
                                font.pixelSize: usage.theme.textBody
                                color: usage.theme.secondaryText
                            }

                            Label {
                                text: reading.expired ? "reset since last reading"
                                                     : usage.agents.window_used_texts[reading.index] ?? ""
                                font.pixelSize: usage.theme.textBody
                                font.bold: !reading.expired
                                color: reading.expired ? usage.theme.secondaryText
                                                      : usage.theme.primaryText
                            }
                        }

                        Rectangle {
                            Layout.fillWidth: true
                            visible: !reading.expired
                            opacity: reading.stale ? 0.6 : 1
                            implicitHeight: 5
                            radius: 3
                            color: usage.theme.control

                            Rectangle {
                                width: parent.width * Math.max(0, Math.min(1, reading.used / 100))
                                height: parent.height
                                radius: 3
                                color: reading.used > 80 ? usage.theme.ember : usage.theme.accent
                            }
                        }

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: usage.theme.gap

                            LongLabel {
                                theme: usage.theme
                                Layout.fillWidth: true
                                text: usage.agents.window_reset_texts[reading.index] ?? ""
                                font.pixelSize: usage.theme.textSmall
                                color: usage.theme.mutedText
                            }

                            // A reading that has gone quiet says how old it is where the eye
                            // lands, since the dimmed figure beside it may no longer be true.
                            Label {
                                visible: reading.stale || reading.expired
                                text: usage.agents.window_age_texts[reading.index] ?? ""
                                font.pixelSize: usage.theme.textSmall
                                font.bold: reading.stale
                                color: reading.stale ? usage.theme.warm : usage.theme.mutedText
                            }
                        }
                    }
                }

                LongLabel {
                    theme: usage.theme
                    Layout.fillWidth: true
                    visible: agent.kind === "totals"
                    text: usage.agents.agent_totals[agent.index] ?? ""
                    font.pixelSize: usage.theme.textBody
                    color: usage.theme.primaryText
                }

                Label {
                    visible: agent.kind === "totals" && text.length > 0
                    text: usage.agents.agent_costs[agent.index] ?? ""
                    font.pixelSize: usage.theme.textBody
                    font.bold: true
                    color: usage.theme.primaryText
                }

                // The no-reading note says what to do, so it is read at secondary weight; the
                // totals note only qualifies the figures above it.
                Label {
                    Layout.fillWidth: true
                    visible: text.length > 0
                    text: usage.agents.agent_notes[agent.index] ?? ""
                    font.pixelSize: agent.kind === "no-reading" ? usage.theme.textBody
                                                                 : usage.theme.textSmall
                    color: agent.kind === "no-reading" ? usage.theme.secondaryText
                                                       : usage.theme.mutedText
                    wrapMode: Text.WordWrap
                }
            }
        }
    }
}
