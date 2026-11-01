import QtQuick
import org.kde.kirigami as Kirigami

import "../code/card-view.mjs" as CardView

// The CPU row's sparkline.
Canvas {
    id: spark

    required property var samples

    readonly property color stroke: Kirigami.Theme.highlightColor

    implicitWidth: 36
    implicitHeight: 14

    onSamplesChanged: requestPaint()
    onStrokeChanged: requestPaint()
    onPaint: {
        const context = getContext("2d");
        context.reset();
        context.strokeStyle = stroke;
        context.lineWidth = 1.4;
        context.beginPath();
        CardView.sparkPoints(samples, width, height).forEach((point, index) => {
            if (index === 0) {
                context.moveTo(point.x, point.y);
            } else {
                context.lineTo(point.x, point.y);
            }
        });
        context.stroke();
    }
}
