import QtQuick

/// Three caverns, each in two lights. The torch is the one Kavverna has always lived in:
/// charcoal and stone under a warm flame, parchment at the mouth. The tide is the same cave
/// flooded: deep blue-slate water with a cold bright accent. The ember is the cave burning
/// down: red heat on warmer darks, clay where the light reaches. Only the light or dark
/// question is asked of the desktop; every colour is Kavverna's own, so the panel looks like
/// itself on anybody's colour scheme.
///
/// Every text-on-surface pair was measured against WCAG AA before its values were frozen;
/// mutedText is decorative by design and sits below it in every palette, torch included.
QtObject {
    id: theme

    /// Unknown means nothing answered, which happens offscreen and in a bare session. A cave is
    /// dark until told otherwise.
    property bool dark: Application.styleHints.colorScheme !== Qt.Light

    /// Which palette dresses the panel. A name nobody knows falls back to the torch, so a
    /// hand-edited settings file cannot leave the panel unreadable.
    property string name: "torch"

    readonly property var palettes: ({
        torch: {
            dark: {
                // Torchlight, and the one colour that stays whatever the desktop says.
                accent: "#E9B44C",
                ember: "#E9573D",
                surface: Qt.rgba(0.102, 0.094, 0.086, 0.97),
                raised: Qt.rgba(1, 1, 1, 0.06),
                sunken: Qt.rgba(1, 1, 1, 0.05),
                hairline: Qt.rgba(1, 1, 1, 0.09),
                control: Qt.rgba(1, 1, 1, 0.08),
                controlDown: Qt.rgba(1, 1, 1, 0.16),
                primaryText: Qt.rgba(1, 1, 1, 0.95),
                secondaryText: Qt.rgba(1, 1, 1, 0.52),
                mutedText: Qt.rgba(1, 1, 1, 0.25)
            },
            light: {
                // Burnt down on parchment: the bright flame reads at 3.4 to 1 there, below
                // what text needs. Ink on parchment starts from a bright ground, so every
                // tint needs more separation than charcoal asked for.
                accent: "#94600F",
                ember: "#B33923",
                surface: Qt.rgba(0.925, 0.894, 0.843, 0.97),
                raised: Qt.rgba(1, 1, 1, 0.72),
                sunken: Qt.rgba(0.30, 0.24, 0.15, 0.11),
                hairline: Qt.rgba(0.30, 0.24, 0.15, 0.24),
                control: Qt.rgba(0.30, 0.24, 0.15, 0.15),
                controlDown: Qt.rgba(0.30, 0.24, 0.15, 0.28),
                primaryText: Qt.rgba(0.13, 0.10, 0.07, 0.95),
                secondaryText: Qt.rgba(0.11, 0.09, 0.06, 0.72),
                mutedText: Qt.rgba(0.11, 0.09, 0.06, 0.50)
            }
        },
        tide: {
            dark: {
                // Deep water: blue-slate ground, moonlit blue accent, rose for the top of
                // the ladder, and text the colour of foam rather than plain white.
                accent: "#7AA2F7",
                ember: "#F7768E",
                surface: Qt.rgba(0.102, 0.106, 0.149, 0.97),
                raised: Qt.rgba(1, 1, 1, 0.06),
                sunken: Qt.rgba(1, 1, 1, 0.05),
                hairline: Qt.rgba(1, 1, 1, 0.09),
                control: Qt.rgba(1, 1, 1, 0.08),
                controlDown: Qt.rgba(1, 1, 1, 0.16),
                primaryText: "#C0CAF5",
                secondaryText: Qt.alpha("#C0CAF5", 0.66),
                mutedText: Qt.alpha("#C0CAF5", 0.34)
            },
            light: {
                // The same water in daylight: cool grey ground, the accent deepened until it
                // reads as text, ink mixed from slate rather than from brown.
                accent: "#2E5FC7",
                ember: "#B02A47",
                surface: Qt.rgba(0.882, 0.886, 0.906, 0.97),
                raised: Qt.rgba(1, 1, 1, 0.72),
                sunken: Qt.rgba(0.14, 0.16, 0.23, 0.11),
                hairline: Qt.rgba(0.14, 0.16, 0.23, 0.24),
                control: Qt.rgba(0.14, 0.16, 0.23, 0.15),
                controlDown: Qt.rgba(0.14, 0.16, 0.23, 0.28),
                primaryText: Qt.alpha("#24283B", 0.95),
                secondaryText: Qt.alpha("#24283B", 0.75),
                mutedText: Qt.alpha("#24283B", 0.52)
            }
        },
        ember: {
            dark: {
                // The fire itself: red-orange flame on charcoal warmed through, whites with
                // the chill taken off, and the torch's amber demoted to the warning rung.
                accent: "#EF6D50",
                ember: "#E9B44C",
                surface: Qt.rgba(0.114, 0.090, 0.082, 0.97),
                raised: Qt.rgba(1, 1, 1, 0.06),
                sunken: Qt.rgba(1, 1, 1, 0.05),
                hairline: Qt.rgba(1, 1, 1, 0.09),
                control: Qt.rgba(1, 1, 1, 0.08),
                controlDown: Qt.rgba(1, 1, 1, 0.16),
                primaryText: Qt.rgba(1.0, 0.97, 0.95, 0.95),
                secondaryText: Qt.rgba(1.0, 0.95, 0.92, 0.54),
                mutedText: Qt.rgba(1.0, 0.95, 0.92, 0.27)
            },
            light: {
                // Fired clay: terracotta ground, the red deepened to brick so it carries
                // text, and dark amber where heat needs naming.
                accent: "#9C2F1E",
                ember: "#8A5A00",
                surface: Qt.rgba(0.933, 0.871, 0.831, 0.97),
                raised: Qt.rgba(1, 1, 1, 0.72),
                sunken: Qt.rgba(0.32, 0.20, 0.15, 0.11),
                hairline: Qt.rgba(0.32, 0.20, 0.15, 0.24),
                control: Qt.rgba(0.32, 0.20, 0.15, 0.15),
                controlDown: Qt.rgba(0.32, 0.20, 0.15, 0.28),
                primaryText: Qt.rgba(0.15, 0.09, 0.07, 0.95),
                secondaryText: Qt.rgba(0.13, 0.08, 0.06, 0.72),
                mutedText: Qt.rgba(0.13, 0.08, 0.06, 0.50)
            }
        }
    })

    readonly property var shade: (palettes[name] ?? palettes.torch)[dark ? "dark" : "light"]

    readonly property color accent: shade.accent
    /// The middle of the ladder: a reading worth noticing, and the mark of a hold in place.
    readonly property color warm: accent
    /// The top of it. Load or heat past the point where noticing is enough.
    readonly property color ember: shade.ember

    readonly property color surface: shade.surface
    readonly property color raised: shade.raised
    readonly property color sunken: shade.sunken
    readonly property color hairline: shade.hairline
    readonly property color control: shade.control
    readonly property color controlDown: shade.controlDown
    readonly property color selected: Qt.alpha(accent, dark ? 0.26 : 0.22)
    /// The fainter wash behind something running, a step below a selection.
    readonly property color glow: Qt.alpha(accent, 0.18)

    readonly property color primaryText: shade.primaryText
    readonly property color secondaryText: shade.secondaryText
    readonly property color mutedText: shade.mutedText

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
