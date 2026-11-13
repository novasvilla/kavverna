import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import "../Shared"

/// The Monitoring page: one card per reading, in the order and folding the person chose. Each
/// card exists once and keeps its place in a one-column grid by its index in that order, so a
/// move re-lays the page without rebuilding a card and losing what it had open.
Item {
    id: section

    required property var theme
    required property var hub
    required property var vitals
    required property var agents
    required property var shows
    /// The applications behind each meter, and the ports they listen on.
    required property var processes
    required property var ports
    /// The page's scroller, run while a card is held near its top or bottom edge so a card can
    /// be carried to a place that is off screen.
    property var flick: null

    /// The card being carried, empty when none is.
    property string carried: ""
    /// The card the carried one would land in front of, empty for the end of the page.
    property string landing: ""
    property point pointer: Qt.point(0, 0)

    readonly property var units: ({
        processor: processorUnit,
        graphics: graphicsUnit,
        network: networkUnit,
        storage: storageUnit,
        agents: agentsUnit,
        ports: portsUnit
    })

    /// The cards a person can see, top to bottom.
    function shownOrder() {
        return section.hub.monitoring_order.filter((card) => section.units[card]?.visible)
    }

    /// The visible card next to `card` in `direction`, or "" when there is none.
    function neighbour(card, direction) {
        const shown = section.shownOrder()
        const at = shown.indexOf(card)
        const next = at + direction
        return at >= 0 && next >= 0 && next < shown.length ? shown[next] : ""
    }

    function moveBy(card, direction) {
        const shown = section.shownOrder()
        const at = shown.indexOf(card)
        if (direction < 0 && at > 0) {
            section.hub.place_monitoring_card(card, shown[at - 1])
        } else if (direction > 0 && at >= 0 && at < shown.length - 1) {
            section.hub.place_monitoring_card(card, shown[at + 2] ?? "")
        }
    }

    function aim(scenePoint) {
        section.pointer = section.mapFromItem(null, scenePoint.x, scenePoint.y)
        const shown = section.shownOrder()
        let before = ""
        for (const card of shown) {
            const unit = section.units[card]
            if (unit.y + unit.height / 2 > section.pointer.y) {
                before = card
                break
            }
        }
        section.landing = before
    }

    /// Dropping a card in front of itself or of the card already after it changes nothing,
    /// so no line is drawn there.
    readonly property bool landingMoves: section.carried.length > 0
        && section.landing !== section.carried
        && section.landing !== section.neighbour(section.carried, 1)

    Layout.fillWidth: true
    implicitHeight: grid.implicitHeight

    FontMetrics {
        id: headingMetrics
        font.pixelSize: section.theme.textSmall
        font.bold: true
    }

    readonly property int headingHeight: Math.max(Math.ceil(headingMetrics.height),
                                                  section.theme.iconSmall)

    /// A card's frame: the heading that folds it on a click and carries it on a drag, and the
    /// body under it.
    component Placed: Item {
        id: placed

        required property string card
        /// What the folded heading says after the title, a reading or two in one line.
        property string summary: ""
        /// The listening ports draw their own heading and fold themselves, so only the grip is
        /// laid over them.
        property bool ownHeading: false
        readonly property int at: section.hub.monitoring_order.indexOf(placed.card)
        readonly property bool folded: section.hub.monitoring_collapsed.indexOf(placed.card) >= 0
        readonly property string title: section.hub.monitoring_titles[placed.at] ?? ""
        default property alias content: body.data

        function fold() {
            section.hub.fold_monitoring_card(placed.card, !placed.folded)
        }

        Layout.row: Math.max(0, placed.at)
        Layout.column: 0
        Layout.fillWidth: true
        implicitHeight: column.implicitHeight
        visible: section.shows(section.hub.monitoring_needs[placed.at] ?? "")
        opacity: section.carried === placed.card ? section.theme.gone : 1

        ColumnLayout {
            id: column
            width: parent.width
            spacing: section.theme.pad

            Pressable {
                id: heading
                theme: section.theme
                visible: !placed.ownHeading
                Layout.fillWidth: true
                implicitHeight: section.headingHeight
                text: placed.title
                leftInset: -section.theme.gapTight
                rightInset: -section.theme.gapTight
                fill: heading.hovered || heading.down ? section.theme.control : "transparent"
                onClicked: placed.fold()

                Accessible.expandable: true
                Accessible.expanded: !placed.folded
                Accessible.description: "Alt and the arrow keys move the card"

                // The keyboard's way to move a card, the same move a drag makes.
                Keys.onPressed: (event) => {
                    if ((event.modifiers & Qt.AltModifier)
                            && (event.key === Qt.Key_Up || event.key === Qt.Key_Down)) {
                        section.moveBy(placed.card, event.key === Qt.Key_Up ? -1 : 1)
                        event.accepted = true
                    }
                }

                contentItem: RowLayout {
                    spacing: section.theme.gapSnug

                    // Inside the heading rather than on the grip over it: a hover handler
                    // laid over the heading would take the hover from it and its wash with it.
                    HoverHandler {
                        cursorShape: Qt.OpenHandCursor
                    }

                    SectionLabel {
                        theme: section.theme
                        text: placed.title.toUpperCase()
                    }

                    LongLabel {
                        theme: section.theme
                        Layout.fillWidth: true
                        visible: placed.folded && placed.summary.length > 0
                        text: placed.summary
                        font.pixelSize: section.theme.textSmall
                        color: section.theme.mutedText
                    }

                    Item {
                        Layout.fillWidth: !placed.folded || placed.summary.length === 0
                        implicitWidth: 0
                    }

                    // Room for the grip drawn over this line.
                    Item {
                        implicitWidth: section.theme.iconSmall
                    }

                    Kirigami.Icon {
                        source: "go-next-symbolic"
                        implicitWidth: section.theme.iconSmall
                        implicitHeight: section.theme.iconSmall
                        isMask: true
                        color: placed.folded ? section.theme.secondaryText : section.theme.accent
                        rotation: placed.folded ? 0 : 90

                        Behavior on rotation {
                            NumberAnimation { duration: section.theme.motionQuick; easing.type: Easing.OutCubic }
                        }
                    }
                }
            }

            ColumnLayout {
                id: body
                Layout.fillWidth: true
                visible: placed.ownHeading || !placed.folded
                spacing: section.theme.pad
            }
        }

        // Laid over the heading rather than inside it, so the ports card's own heading carries
        // a card the same way. The drag only takes the pointer once it has moved past the
        // threshold, which cancels the press underneath: a click folds, a drag never does.
        Item {
            id: grip
            width: parent.width
            height: section.headingHeight

            // A press inside a scroller is the scroller's to take, and a vertical drag is
            // exactly what it would take, so it holds still while a heading is pressed.
            PointHandler {
                onActiveChanged: if (section.flick) section.flick.interactive = !active
            }

            DragHandler {
                id: carry
                target: null
                xAxis.enabled: false
                cursorShape: Qt.ClosedHandCursor
                onActiveChanged: {
                    if (active) {
                        section.carried = placed.card
                        section.aim(carry.centroid.scenePosition)
                    } else {
                        const moves = section.landingMoves
                        const before = section.landing
                        section.carried = ""
                        if (moves) {
                            section.hub.place_monitoring_card(placed.card, before)
                        }
                    }
                }
                onCentroidChanged: if (active) section.aim(carry.centroid.scenePosition)
            }

            Kirigami.Icon {
                anchors.right: parent.right
                anchors.rightMargin: section.theme.iconSmall + section.theme.gapSnug
                anchors.verticalCenter: parent.verticalCenter
                source: "drag-handle-symbolic"
                implicitWidth: section.theme.iconSmall
                implicitHeight: section.theme.iconSmall
                isMask: true
                color: section.theme.mutedText
                opacity: heading.hovered || section.carried === placed.card ? 1 : 0.5
            }
        }
    }

    component Meter: ColumnLayout {
        id: meter

        required property var theme
        property string label
        property string value
        /// Which `open_fold` lists the applications behind this meter, empty for none.
        property string fold: ""
        property string foldTitle: ""
        property real fraction: 0
        property color tint: theme.accent
        /// The last couple of minutes of this reading. Left out for a meter whose past says
        /// nothing, such as how much of memory applications are holding.
        property var past: []

        Layout.fillWidth: true
        spacing: 3

        FoldHeading {
            theme: meter.theme
            label: meter.label
            value: meter.value
            foldable: meter.fold.length > 0
            open: meter.fold.length > 0 && section.processes.open_fold === meter.fold
            onClicked: section.processes.open_fold = open ? "" : meter.fold
        }

        Trace {
            theme: meter.theme
            Layout.fillWidth: true
            implicitHeight: 26
            visible: meter.past && meter.past.length > 1
            readings: meter.past
            tint: meter.tint
        }

        Rectangle {
            Layout.fillWidth: true
            implicitHeight: 5
            radius: 3
            color: theme.control

            Rectangle {
                width: parent.width * Math.max(0, Math.min(1, meter.fraction))
                height: parent.height
                radius: 3
                color: meter.tint
            }
        }

        ApplicationFold {
            theme: meter.theme
            view: section.processes
            resource: meter.fold
            title: meter.foldTitle
        }
    }

    GridLayout {
        id: grid
        width: parent.width
        columns: 1
        rowSpacing: section.theme.pad

        Placed {
            id: processorUnit
            card: "processor"
            summary: "CPU " + section.vitals.cpu_load_text + "  ·  " + section.vitals.memory_text

            Card {
                theme: section.theme
                spacing: section.theme.gap

                ColumnLayout {
                    Layout.fillWidth: true
                    visible: section.vitals.processor_name.length > 0
                    spacing: 1

                    LongLabel {
                        theme: section.theme
                        Layout.fillWidth: true
                        text: section.vitals.processor_name
                        font.pixelSize: section.theme.textBody
                        font.bold: true
                        color: section.theme.primaryText
                        elide: Text.ElideRight
                    }

                    LongLabel {
                        theme: section.theme
                        Layout.fillWidth: true
                        text: section.vitals.processor_detail
                        font.pixelSize: section.theme.textFine
                        color: section.theme.mutedText
                        elide: Text.ElideRight
                    }
                }

                Meter {
                    theme: section.theme
                    fold: "cpu"
                    foldTitle: "Apps using CPU"
                    label: "CPU  ·  " + section.vitals.cpu_temperature_text
                    value: section.vitals.cpu_load_text
                    fraction: section.vitals.cpu_load
                    past: section.vitals.cpu_history
                    tint: section.vitals.cpu_load > 0.9 ? section.theme.ember
                        : section.vitals.cpu_load > 0.7 ? section.theme.warm
                                                        : section.theme.accent
                }

                RowLayout {
                    Layout.fillWidth: true
                    spacing: 2
                    visible: section.vitals.core_loads.length > 0

                    Repeater {
                        model: section.vitals.core_loads.length

                        delegate: Rectangle {
                            required property int index

                            Layout.fillWidth: true
                            implicitHeight: 16
                            radius: 2
                            color: section.theme.control

                            Rectangle {
                                anchors.bottom: parent.bottom
                                width: parent.width
                                height: parent.height * section.vitals.core_loads[parent.index]
                                radius: 2
                                color: section.theme.accent
                            }
                        }
                    }
                }

                Meter {
                    theme: section.theme
                    label: "Memory"
                    fold: "memory"
                    foldTitle: "Apps using memory"
                    value: section.vitals.memory_text
                    fraction: section.vitals.memory_used
                    past: section.vitals.memory_history
                }

                Meter {
                    theme: section.theme
                    label: "Held by applications"
                    value: section.vitals.memory_apps_text
                    fraction: section.vitals.memory_apps
                    tint: section.theme.secondaryText
                }

                RowLayout {
                    Layout.fillWidth: true

                    Label {
                        Layout.fillWidth: true
                        text: "Pressure"
                        font.pixelSize: section.theme.textBody
                        color: section.theme.secondaryText
                    }

                    Label {
                        text: section.vitals.pressure_text
                        font.pixelSize: section.theme.textBody
                        color: section.theme.primaryText
                    }
                }

                RowLayout {
                    Layout.fillWidth: true

                    Label {
                        Layout.fillWidth: true
                        text: "Compressed swap"
                        font.pixelSize: section.theme.textBody
                        color: section.theme.secondaryText
                    }

                    Label {
                        text: section.vitals.swap_text
                        font.pixelSize: section.theme.textBody
                        color: section.theme.primaryText
                    }
                }
            }
        }

        Placed {
            id: graphicsUnit
            card: "graphics"
            summary: (section.vitals.card_names[section.vitals.chosen_card] ?? "GPU") + "  "
                     + section.vitals.gpu_usage_text

            Card {
                theme: section.theme
                spacing: section.theme.gap

                RowLayout {
                    Layout.fillWidth: true
                    spacing: section.theme.gapTight
                    visible: section.vitals.card_names.length > 1

                    Repeater {
                        model: section.vitals.card_names

                        delegate: PillButton {
                            required property int index
                            required property string modelData

                            theme: section.theme
                            active: section.vitals.chosen_card === index
                            Layout.fillWidth: true
                            compact: true
                            text: modelData
                            onClicked: section.vitals.choose_card(index)
                        }
                    }
                }

                // The GPU fold reads the card the picker shows, by the name the picker shows it
                // under, and is told before anything opens it, so one card's apps never land
                // under another.
                Binding {
                    target: section.processes
                    property: "card_name"
                    value: section.vitals.card_names[section.vitals.chosen_card] ?? ""
                }

                Meter {
                    theme: section.theme
                    fold: "gpu"
                    foldTitle: "Apps using "
                               + (section.vitals.card_names[section.vitals.chosen_card] ?? "the GPU")
                    label: "GPU  ·  " + section.vitals.gpu_temperature_text
                           + "  ·  " + section.vitals.gpu_power_text
                    value: section.vitals.gpu_usage_text
                    fraction: section.vitals.gpu_usage
                    past: section.vitals.gpu_history
                }

                Meter {
                    theme: section.theme
                    label: "VRAM"
                    value: section.vitals.vram_text
                    fraction: section.vitals.vram_used
                    tint: section.vitals.vram_used > 0.9 ? section.theme.ember : section.theme.warm
                }
            }
        }

        Placed {
            id: networkUnit
            card: "network"
            summary: section.vitals.network_names.length > 0
                     ? section.vitals.network_names[0] + "  " + section.vitals.network_received[0]
                       + "  " + section.vitals.network_sent[0]
                     : section.vitals.network_status

            Card {
                theme: section.theme
                spacing: section.theme.gap

                Label {
                    Layout.fillWidth: true
                    visible: section.vitals.network_status.length > 0
                    text: section.vitals.network_status
                    font.pixelSize: section.theme.textBody
                    color: section.theme.secondaryText
                    wrapMode: Text.WordWrap
                }

                Repeater {
                    model: section.vitals.network_names.length

                    delegate: ColumnLayout {
                        id: interfaceRow
                        required property int index
                        Layout.fillWidth: true
                        spacing: 3

                        LongLabel {
                            theme: section.theme
                            Layout.fillWidth: true
                            text: section.vitals.network_names[interfaceRow.index]
                            font.pixelSize: section.theme.textStrong
                            font.bold: true
                            color: section.theme.primaryText
                        }

                        // Half the row each, so a fast link's longer figure cannot shove the
                        // other direction off the card.
                        RowLayout {
                            Layout.fillWidth: true
                            spacing: section.theme.gap

                            LongLabel {
                                theme: section.theme
                                Layout.fillWidth: true
                                Layout.preferredWidth: 1
                                text: section.vitals.network_received[interfaceRow.index]
                                font.pixelSize: section.theme.textBody
                                color: section.theme.secondaryText
                            }
                            LongLabel {
                                theme: section.theme
                                Layout.fillWidth: true
                                Layout.preferredWidth: 1
                                text: section.vitals.network_sent[interfaceRow.index]
                                font.pixelSize: section.theme.textBody
                                color: section.theme.secondaryText
                            }
                        }

                        LongLabel {
                            theme: section.theme
                            Layout.fillWidth: true
                            text: section.vitals.network_totals[interfaceRow.index]
                            font.pixelSize: section.theme.textSmall
                            color: section.theme.mutedText
                        }
                    }
                }
            }
        }

        Placed {
            id: storageUnit
            card: "storage"
            summary: section.vitals.disk_names.length > 0
                     ? section.vitals.disk_names[0] + "  " + section.vitals.disk_spaces[0]
                     : section.vitals.disk_status

            Card {
                theme: section.theme
                spacing: section.theme.gap

                Label {
                    Layout.fillWidth: true
                    visible: section.vitals.disk_status.length > 0
                    text: section.vitals.disk_status
                    font.pixelSize: section.theme.textBody
                    color: section.theme.secondaryText
                    wrapMode: Text.WordWrap
                }

                Repeater {
                    model: section.vitals.disk_names.length

                    delegate: ColumnLayout {
                        id: diskRow
                        required property int index
                        Layout.fillWidth: true
                        spacing: 3

                        LongLabel {
                            theme: section.theme
                            Layout.fillWidth: true
                            text: section.vitals.disk_names[diskRow.index]
                            font.pixelSize: section.theme.textStrong
                            font.bold: true
                            color: section.theme.primaryText
                        }
                        LongLabel {
                            theme: section.theme
                            Layout.fillWidth: true
                            text: section.vitals.disk_spaces[diskRow.index]
                            font.pixelSize: section.theme.textBody
                            color: section.theme.secondaryText
                        }
                        LongLabel {
                            theme: section.theme
                            Layout.fillWidth: true
                            text: section.vitals.disk_rates[diskRow.index]
                            font.pixelSize: section.theme.textSmall
                            color: section.theme.mutedText
                        }
                    }
                }

                // One fold for the card rather than one per disk: the applications are counted
                // by what they read and write, not by which disk it lands on.
                FoldHeading {
                    theme: section.theme
                    label: "Disk activity"
                    open: section.processes.open_fold === "disk"
                    onClicked: section.processes.open_fold = open ? "" : "disk"
                }

                ApplicationFold {
                    theme: section.theme
                    view: section.processes
                    resource: "disk"
                    title: "Apps using the disk"
                }
            }
        }

        Placed {
            id: agentsUnit
            card: "agents"
            summary: section.agents.summary

            AgentUsageCard {
                theme: section.theme
                agents: section.agents
            }
        }

        Placed {
            id: portsUnit
            card: "ports"
            ownHeading: true

            PortsCard {
                id: portsCard
                theme: section.theme
                view: section.ports
                // The card folds itself from its own heading; the choice is kept with the rest.
                onOpenChanged: if (portsCard.open === portsUnit.folded) {
                    section.hub.fold_monitoring_card("ports", !portsCard.open)
                }
            }

            Binding {
                target: portsCard
                property: "open"
                value: !portsUnit.folded
            }
        }
    }

    // Drawn over the grid rather than in it, so showing the line never re-lays the page.
    Rectangle {
        visible: section.landingMoves
        x: 0
        width: section.width
        height: 2
        radius: 1
        color: section.theme.accent
        y: {
            const unit = section.units[section.landing]
            if (unit) {
                return unit.y - Math.round(section.theme.pad / 2) - 1
            }
            const shown = section.shownOrder()
            const last = section.units[shown[shown.length - 1]]
            return last ? last.y + last.height + Math.round(section.theme.pad / 2) - 1 : 0
        }
    }

    // The title travels with the hand, since the card itself stays put until it is dropped.
    Rectangle {
        visible: section.carried.length > 0
        x: Math.max(0, Math.min(section.width - width, section.pointer.x - width / 2))
        y: section.pointer.y - height / 2
        width: carriedLabel.implicitWidth + 2 * section.theme.gap
        height: section.theme.buttonHeight
        radius: section.theme.radiusSmall
        color: section.theme.surface
        border.width: 1
        border.color: section.theme.accent

        SectionLabel {
            id: carriedLabel
            anchors.centerIn: parent
            theme: section.theme
            text: (section.hub.monitoring_titles[section.hub.monitoring_order.indexOf(section.carried)] ?? "").toUpperCase()
            color: section.theme.primaryText
        }
    }

    // Near an edge of the scroller the page moves under the held card, a little each tick.
    Timer {
        interval: 16
        repeat: true
        running: section.carried.length > 0 && section.flick !== null
        onTriggered: {
            const flick = section.flick
            const inView = section.mapToItem(flick, section.pointer.x, section.pointer.y).y
            const edge = 2 * section.theme.buttonHeight
            const most = Math.max(0, flick.contentHeight - flick.height)
            let shift = 0
            if (inView < edge) {
                shift = -Math.ceil((edge - inView) / 4)
            } else if (inView > flick.height - edge) {
                shift = Math.ceil((inView - flick.height + edge) / 4)
            }
            const to = Math.max(0, Math.min(most, flick.contentY + shift))
            if (to !== flick.contentY) {
                const moved = to - flick.contentY
                flick.contentY = to
                // The hand stayed where it was while the page slid under it.
                section.pointer = Qt.point(section.pointer.x, section.pointer.y + moved)
                const scene = section.mapToItem(null, section.pointer.x, section.pointer.y)
                section.aim(scene)
            }
        }
    }
}
