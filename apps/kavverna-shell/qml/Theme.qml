import QtQuick

/// A cavern in two lights: charcoal and stone when the desktop is dark, the same cavern at its
/// mouth when it is light. Only the light or dark question is asked of the desktop; the colours
/// are Kavverna's own, so the panel looks like itself on anybody's colour scheme.
QtObject {
    id: theme

    /// Unknown means nothing answered, which happens offscreen and in a bare session. A cave is
    /// dark until told otherwise.
    property bool dark: Application.styleHints.colorScheme !== Qt.Light

    /// Torchlight, and the one colour that stays whatever the desktop says. Burnt down on
    /// parchment: the bright version reads at 3.4 to 1 there, which is below what text needs.
    readonly property color accent: dark ? "#E9B44C" : "#94600F"
    /// The middle of the ladder: a reading worth noticing, and the mark of a hold in place.
    readonly property color warm: accent
    /// The top of it. Load or heat past the point where noticing is enough.
    readonly property color ember: dark ? "#E9573D" : "#B33923"

    // Light needs more separation than dark does. Ink on parchment starts from a bright ground,
    // so a tint that reads clearly against charcoal disappears against paper.
    readonly property color surface: dark ? Qt.rgba(0.102, 0.094, 0.086, 0.97)
                                          : Qt.rgba(0.925, 0.894, 0.843, 0.97)

    readonly property color raised: dark ? Qt.rgba(1, 1, 1, 0.06) : Qt.rgba(1, 1, 1, 0.72)
    readonly property color sunken: dark ? Qt.rgba(1, 1, 1, 0.05) : Qt.rgba(0.30, 0.24, 0.15, 0.11)
    readonly property color hairline: dark ? Qt.rgba(1, 1, 1, 0.09)
                                           : Qt.rgba(0.30, 0.24, 0.15, 0.24)
    readonly property color control: dark ? Qt.rgba(1, 1, 1, 0.08)
                                          : Qt.rgba(0.30, 0.24, 0.15, 0.15)
    readonly property color controlDown: dark ? Qt.rgba(1, 1, 1, 0.16)
                                              : Qt.rgba(0.30, 0.24, 0.15, 0.28)
    readonly property color selected: Qt.alpha(accent, dark ? 0.26 : 0.22)
    /// The fainter wash behind something running, a step below a selection.
    readonly property color glow: Qt.alpha(accent, 0.18)

    readonly property color primaryText: dark ? Qt.rgba(1, 1, 1, 0.95)
                                              : Qt.rgba(0.13, 0.10, 0.07, 0.95)
    readonly property color secondaryText: dark ? Qt.rgba(1, 1, 1, 0.52)
                                                : Qt.rgba(0.11, 0.09, 0.06, 0.72)
    readonly property color mutedText: dark ? Qt.rgba(1, 1, 1, 0.25)
                                            : Qt.rgba(0.11, 0.09, 0.06, 0.50)

    // Only what is genuinely repeated. A one-off pixel value doing a local job, the seven pixel
    // dot or the one pixel gap between a title and its detail, is geometry rather than scale and
    // stays where it is used.
    readonly property int gapTight: 4
    readonly property int gapSnug: 6
    readonly property int gap: 10
    /// What a card holds itself away from its own edge, and what one card keeps from the next.
    readonly property int pad: 12

    readonly property int radiusSmall: 6
    readonly property int radius: 10

    readonly property int textFine: 9
    readonly property int textSmall: 10
    readonly property int textBody: 11
    readonly property int textStrong: 13
    readonly property int textTitle: 16
}
