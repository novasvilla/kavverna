import QtQuick
import QtQuick.Layouts

/// The recessed surface everything on a page sits in. It owns its padding: children land in a
/// padded column, so a card is its content and a spacing rather than a height formula and a
/// margins pair restated at every use. One that needs a height of its own, like the scrolling
/// list, still just sets it.
Rectangle {
    id: card

    required property var theme
    property alias spacing: inner.spacing
    default property alias content: inner.data

    radius: theme.radius
    color: theme.raised
    Layout.fillWidth: true
    implicitHeight: inner.implicitHeight + theme.pad * 2

    ColumnLayout {
        id: inner
        anchors.fill: parent
        anchors.margins: card.theme.pad
    }
}
