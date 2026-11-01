import QtQuick
import org.kde.notification

import "../code/widget.mjs" as Allot

// Sends Allot's notifications through KNotification, whose notifyrc the widget copies at start.
Item {
    id: sender

    required property string iconDirectory

    function send(fields) {
        const made = notification.createObject(sender, {
            title: fields.title,
            text: fields.body,
            iconName: sender.iconDirectory + fields.icon,
        });
        made.actions = fields.buttons.map((button) => {
            const action = notificationAction.createObject(made, { label: button.label });
            action.activated.connect(button.pressed);
            return action;
        });
        const activated = notificationAction.createObject(made, { label: "Show" });
        activated.activated.connect(fields.activated);
        made.defaultAction = activated;
        made.closed.connect(() => {
            fields.closed();
            made.destroy();
        });
        made.sendEvent();
        return made;
    }

    Component {
        id: notification

        Notification {
            componentName: Allot.NOTIFY_COMPONENT
            eventId: Allot.NOTIFY_EVENT
            autoDelete: false
        }
    }

    Component {
        id: notificationAction

        NotificationAction {}
    }
}
