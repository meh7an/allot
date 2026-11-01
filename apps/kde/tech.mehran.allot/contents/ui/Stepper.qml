import QtQuick
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.kde.plasma.components as PlasmaComponents

import "../code/card-view.mjs" as CardView

// The Drain / Quiet / Run stepper of a benchmark card: a thin bar over a label per step.
RowLayout {
    id: stepper

    required property var steps
    required property color color
    required property bool animate

    readonly property string current: steps === null ? "drain" : steps.current
    readonly property int quietTicks: steps === null ? 0 : steps.quietTicks
    readonly property real runFraction: steps === null ? 0 : steps.runFraction
    readonly property color upcoming: CardView.withAlpha(Kirigami.Theme.textColor, 0.12)

    function stepColor(step) {
        switch (CardView.stepState(step, current)) {
        case "current":
            return color;
        case "past":
            return CardView.withAlpha(color, 0.35);
        default:
            return upcoming;
        }
    }

    function labelOpacity(step) {
        switch (CardView.stepState(step, current)) {
        case "current":
            return 1;
        case "past":
            return 0.8;
        default:
            return 0.55;
        }
    }

    spacing: Kirigami.Units.smallSpacing * 1.5

    component StepLabel: PlasmaComponents.Label {
        font: Kirigami.Theme.smallFont
    }

    component StepBar: Rectangle {
        Layout.fillWidth: true
        implicitHeight: 3
        radius: 1.5
    }

    ColumnLayout {
        Layout.fillWidth: true
        Layout.preferredWidth: 0
        Layout.horizontalStretchFactor: 10
        spacing: 2

        StepBar {
            color: stepper.stepColor("drain")
        }

        StepLabel {
            text: "Drain"
            opacity: stepper.labelOpacity("drain")
        }
    }

    ColumnLayout {
        Layout.fillWidth: true
        Layout.preferredWidth: 0
        Layout.horizontalStretchFactor: 14
        spacing: 2

        RowLayout {
            Layout.fillWidth: true
            spacing: 2

            Repeater {
                model: CardView.QUIET_TICKS

                StepBar {
                    required property int index

                    color: index < stepper.quietTicks ? stepper.stepColor("quiet")
                        : stepper.upcoming
                }
            }
        }

        StepLabel {
            text: "Quiet"
            opacity: stepper.labelOpacity("quiet")
        }
    }

    ColumnLayout {
        Layout.fillWidth: true
        Layout.preferredWidth: 0
        Layout.horizontalStretchFactor: 10
        spacing: 2

        StepBar {
            color: stepper.upcoming

            Rectangle {
                property real share: stepper.runFraction

                width: parent.width * share
                height: parent.height
                radius: parent.radius
                color: stepper.stepColor("run")

                Behavior on share {
                    enabled: stepper.animate

                    NumberAnimation {
                        duration: CardView.scaledMs(CardView.STEP_RUN_MS,
                            Kirigami.Units.longDuration)
                        easing.type: Easing.Linear
                    }
                }
            }
        }

        StepLabel {
            text: "Run"
            opacity: stepper.labelOpacity("run")
        }
    }
}
