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
    onInViewChanged: if (usage.inView) {
        usage.now = Date.now()
        usage.agents.refresh()
    }

    /// The clock the reset wording counts from, moved on while the card is seen so "in 35 min"
    /// does not stand still.
    property real now: Date.now()

    Timer {
        interval: 30000
        repeat: true
        running: usage.inView
        onTriggered: usage.now = Date.now()
    }

    readonly property var weekdays: ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday",
                                     "Friday", "Saturday"]
    readonly property var months: ["January", "February", "March", "April", "May", "June", "July",
                                   "August", "September", "October", "November", "December"]

    /// A reset in the words a person would use: minutes when it is close, today or tomorrow
    /// within the day, and the weekday beyond, with the time in the desktop's own clock format.
    function resetWords(at, now) {
        if (!(at > 0)) {
            return "Reset time unknown"
        }
        const left = at - now
        if (left < 60000) {
            return "Resets in under a minute"
        }
        if (left < 3600000) {
            return "Resets in " + Math.floor(left / 60000) + " min"
        }
        const when = new Date(at)
        const time = when.toLocaleTimeString(Qt.locale(), Locale.ShortFormat)
        if (left < 20 * 3600000) {
            const today = new Date(now)
            const tomorrow = new Date(now)
            tomorrow.setDate(today.getDate() + 1)
            if (when.toDateString() === today.toDateString()) {
                return "Resets today, " + time
            }
            if (when.toDateString() === tomorrow.toDateString()) {
                return "Resets tomorrow, " + time
            }
        }
        // The words are English like the rest of the panel, and only the clock follows the
        // desktop's locale. A weekday names the coming week only; further out it would point
        // at the wrong one, so the date is given instead.
        const day = left < 6 * 24 * 3600000
            ? usage.weekdays[when.getDay()]
            : when.getDate() + " " + usage.months[when.getMonth()]
        return "Resets " + day + ", " + time
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
                readonly property string title: usage.agents.agent_titles[agent.index] ?? ""
                /// One line in place of the rows, when the rows would only say nothing is
                /// current.
                readonly property string brief: usage.agents.agent_briefs[agent.index] ?? ""
                readonly property bool folded: agent.brief.length > 0
                readonly property string note: usage.agents.agent_notes[agent.index] ?? ""
                property bool noteOpen: false

                Layout.fillWidth: true
                spacing: usage.theme.gapSnug

                RowLayout {
                    Layout.fillWidth: true
                    visible: agent.folded
                    spacing: usage.theme.gapSnug

                    LongLabel {
                        theme: usage.theme
                        Layout.fillWidth: true
                        text: agent.title + "  ·  " + agent.brief
                        font.pixelSize: usage.theme.textBody
                        color: usage.theme.mutedText
                    }

                    // A folded agent still waiting on its setup keeps the instructions one
                    // press away rather than across the card.
                    IconButton {
                        theme: usage.theme
                        visible: agent.kind === "no-reading" && agent.note.length > 0
                        source: "help-about-symbolic"
                        size: usage.theme.iconSmall
                        text: agent.noteOpen ? "Hide how to connect" : "How to connect"
                        onClicked: agent.noteOpen = !agent.noteOpen
                    }
                }

                Label {
                    Layout.fillWidth: true
                    visible: agent.folded && agent.noteOpen
                    text: agent.note
                    font.pixelSize: usage.theme.textSmall
                    color: usage.theme.secondaryText
                    wrapMode: Text.WordWrap
                }

                RowLayout {
                    Layout.fillWidth: true
                    visible: !agent.folded
                    spacing: usage.theme.gap

                    LongLabel {
                        theme: usage.theme
                        Layout.fillWidth: true
                        text: agent.title
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
                    model: agent.kind === "allowance" && !agent.folded
                           ? usage.agents.window_agents.length : 0

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
                                color: usage.theme.primaryText
                            }

                            Label {
                                text: reading.expired
                                      ? "Reset since last reading"
                                      : (usage.agents.window_used_texts[reading.index] ?? "") + " used"
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

                        Label {
                            Layout.fillWidth: true
                            visible: !reading.expired
                            text: usage.resetWords(usage.agents.window_resets_at[reading.index] ?? 0,
                                                   usage.now)
                            font.pixelSize: usage.theme.textSmall
                            color: usage.theme.mutedText
                            elide: Text.ElideRight
                        }
                    }
                }

                LongLabel {
                    theme: usage.theme
                    Layout.fillWidth: true
                    visible: agent.kind === "totals" && !agent.folded
                    text: usage.agents.agent_totals[agent.index] ?? ""
                    font.pixelSize: usage.theme.textBody
                    color: usage.theme.primaryText
                }

                Label {
                    visible: agent.kind === "totals" && !agent.folded && text.length > 0
                    text: usage.agents.agent_costs[agent.index] ?? ""
                    font.pixelSize: usage.theme.textBody
                    font.bold: true
                    color: usage.theme.primaryText
                }

                // The no-reading note says what to do, so it is read at secondary weight; the
                // totals note only qualifies the figures above it.
                Label {
                    Layout.fillWidth: true
                    visible: !agent.folded && text.length > 0
                    text: agent.note
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
