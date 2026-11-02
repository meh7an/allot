import QtQuick

import "../code/card-view.mjs" as CardView

// A bar's share of its width. It follows `target` and glides only over moves big enough to see,
// since QML drives a glide from the shell's main thread, frame by frame.
Item {
    id: barShare

    required property real target
    required property real barWidth
    required property bool animate
    required property int duration
    required property int easingType

    property real value: 0
    property real moved: 0
    property bool glides: false

    onTargetChanged: {
        const move = CardView.barMove(moved, target, barWidth);
        if (move === "stay") {
            return;
        }
        glides = move === "glide" && animate;
        moved = target;
        value = target;
    }

    Component.onCompleted: {
        moved = target;
        value = target;
    }

    Behavior on value {
        enabled: barShare.glides

        NumberAnimation {
            duration: barShare.duration
            easing.type: barShare.easingType
        }
    }
}
