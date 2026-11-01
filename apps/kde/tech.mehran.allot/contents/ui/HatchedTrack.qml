import QtQuick
import org.kde.kirigami as Kirigami

// A bar's track: plain up to `headroom`, then hatched, which leases cannot take. Painted once per
// size, since a Rectangle has neither hatching nor per-corner radii on Qt 6.6.
Canvas {
    id: track

    required property real headroom
    property real radius: 0

    readonly property color ink: Kirigami.Theme.textColor

    function inkAt(alpha) {
        return Qt.rgba(track.ink.r, track.ink.g, track.ink.b, alpha);
    }

    onWidthChanged: requestPaint()
    onHeightChanged: requestPaint()
    onHeadroomChanged: requestPaint()
    onInkChanged: requestPaint()
    onPaint: {
        const step = 4;
        const context = getContext("2d");
        const start = width * headroom;
        context.reset();
        context.beginPath();
        context.roundedRect(0, 0, width, height, radius, radius);
        context.clip();
        context.fillStyle = inkAt(0.08);
        context.fillRect(0, 0, start, height);
        context.fillStyle = inkAt(0.05);
        context.fillRect(start, 0, width - start, height);
        context.beginPath();
        context.rect(start, 0, width - start, height);
        context.clip();
        context.strokeStyle = inkAt(0.22);
        context.lineWidth = 1.5;
        context.beginPath();
        for (let x = start - height; x < width; x += step) {
            context.moveTo(x, height);
            context.lineTo(x + height, 0);
        }
        context.stroke();
    }
}
