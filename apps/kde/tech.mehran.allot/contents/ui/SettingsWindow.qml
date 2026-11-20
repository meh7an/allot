import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

import "../code/settings-view.mjs" as View

// The Settings window (DESIGN.md D1, section 10 KDE): a window of the widget's own, built on the
// first open and hidden on close, so a reopen shows the same page (D14). The page list is on the
// left with the save status under it; the page on the right. What the pages show comes from the
// shared SettingsWindowModel; `revision` moves on whenever it changes, and every view reads it.
Kirigami.ApplicationWindow {
    id: window

    required property var widget
    required property var timers
    // The Panel page's state, mirrored from the widget item.
    property var panelSettings: ({ meters: {}, notifications: {} })
    property bool sharesMemory: false
    property bool notificationsOff: false
    property bool doNotDisturb: false

    property var model: null
    property int revision: 0
    property int pageIndex: 0
    readonly property var pages: [View.PANEL_PAGE_ENTRY].concat(View.ALLOTD_PAGES)
    readonly property bool readOnly: {
        window.revision;
        return window.model === null || window.model.session.readOnly;
    }
    readonly property string saveStatus: {
        window.revision;
        return window.model === null ? "" : window.model.pageState("admission").status;
    }

    function changed() {
        window.revision += 1;
    }

    // The views pass this on as the shared code's number formatter; QML has no Intl.
    function number(value) {
        return View.localeNumber(value, (shown, decimals) =>
            Number(shown).toLocaleString(Qt.locale(), "f", decimals));
    }

    // Raises a window that is open already; DESIGN.md section 9.
    function present() {
        window.show();
        window.raise();
        window.requestActivate();
    }

    function notice(text, undo) {
        if (undo === null || undo === undefined) {
            window.showPassiveNotification(text, 5000);
            return;
        }
        window.showPassiveNotification(text, 10000, "Undo", () => {
            undo();
            window.changed();
        });
    }

    function confirm(confirmation, run) {
        confirmDialog.ask(confirmation, run);
    }

    function resetSetting(setting) {
        const reset = View.resetSetting(setting, window.number);
        window.notice(reset.text, reset.undo);
        window.changed();
    }

    function openPath(path) {
        Qt.openUrlExternally("file://" + encodeURI(path));
    }

    function openSizeDialog(table, draft) {
        sizeDialog.edit(table, draft);
    }

    function openRuleDialog(rules, draft) {
        ruleDialog.edit(rules, draft);
    }

    title: "Allot Settings"
    width: Kirigami.Units.gridUnit * 50
    height: Kirigami.Units.gridUnit * 36
    minimumWidth: Kirigami.Units.gridUnit * 32
    minimumHeight: Kirigami.Units.gridUnit * 24

    pageStack.globalToolBar.style: Kirigami.ApplicationHeaderStyle.None

    onVisibleChanged: {
        if (window.model !== null) {
            // Hiding sends a pending change first (M0.3).
            window.model.setVisible(window.visible);
        }
        if (window.visible) {
            window.widget.refreshNotificationState();
        }
    }
    onActiveChanged: {
        if (window.active && window.model !== null) {
            window.model.refresh();
        }
    }
    onPageIndexChanged: {
        if (window.model !== null) {
            window.model.showPage(window.pages[window.pageIndex].id);
        }
    }

    Component.onCompleted: {
        const host = {
            confirm: (confirmation, run) => window.confirm(confirmation, run),
            notify: (text) => window.notice(text, null),
            openPath: (path) => window.openPath(path),
            now: () => Date.now(),
        };
        window.widget.settingsWindow(host, window.number).then((model) => {
            model.onChange = () => window.changed();
            window.model = model;
            model.showPage(window.pages[window.pageIndex].id);
            model.setVisible(window.visible);
            window.changed();
        });
    }

    pageStack.initialPage: Kirigami.Page {
        padding: 0

        RowLayout {
            anchors.fill: parent
            spacing: 0

            ColumnLayout {
                Layout.fillHeight: true
                Layout.preferredWidth: Kirigami.Units.gridUnit * 11
                spacing: 0

                QQC2.ScrollView {
                    Layout.fillWidth: true
                    Layout.fillHeight: true

                    ListView {
                        id: pageList

                        model: window.pages
                        currentIndex: window.pageIndex
                        keyNavigationEnabled: true
                        activeFocusOnTab: true
                        Accessible.role: Accessible.List
                        Accessible.name: "Pages"
                        onCurrentIndexChanged: window.pageIndex = pageList.currentIndex

                        delegate: QQC2.ItemDelegate {
                            required property var modelData
                            required property int index

                            width: ListView.view.width
                            text: modelData.label
                            icon.name: modelData.kdeIcon
                            highlighted: ListView.isCurrentItem
                            onClicked: window.pageIndex = index
                        }
                    }
                }

                QQC2.Label {
                    Layout.fillWidth: true
                    Layout.margins: Kirigami.Units.largeSpacing
                    text: window.saveStatus
                    visible: text !== ""
                    wrapMode: Text.Wrap
                    font: Kirigami.Theme.smallFont
                    Accessible.role: Accessible.StaticText
                    Accessible.name: text
                }
            }

            Kirigami.Separator {
                Layout.fillHeight: true
            }

            StackLayout {
                Layout.fillWidth: true
                Layout.fillHeight: true
                currentIndex: window.pageIndex

                // One child per page, in the list's order: StackLayout counts its children, so the
                // pages are written out rather than repeated.
                PanelPage {
                    host: window
                }

                AllotdPage {
                    host: window
                    entry: View.ALLOTD_PAGES[0]
                }

                AllotdPage {
                    host: window
                    entry: View.ALLOTD_PAGES[1]
                }

                AllotdPage {
                    host: window
                    entry: View.ALLOTD_PAGES[2]
                }

                AllotdPage {
                    host: window
                    entry: View.ALLOTD_PAGES[3]
                }

                AllotdPage {
                    host: window
                    entry: View.ALLOTD_PAGES[4]
                }

                AllotdPage {
                    host: window
                    entry: View.ALLOTD_PAGES[5]
                }
            }
        }
    }

    Kirigami.PromptDialog {
        id: confirmDialog

        // Held inside an object: QML properties do not take a function itself.
        property var pending: null

        function ask(confirmation, run) {
            confirmDialog.title = confirmation.title;
            confirmDialog.subtitle = confirmation.body;
            confirmAction.text = confirmation.button;
            confirmDialog.pending = { run: run };
            confirmDialog.open();
        }

        standardButtons: Kirigami.Dialog.NoButton
        onClosed: confirmDialog.pending = null
        customFooterActions: [
            Kirigami.Action {
                id: confirmAction

                icon.name: "dialog-warning"
                onTriggered: {
                    const pending = confirmDialog.pending;
                    confirmDialog.close();
                    if (pending !== null) {
                        pending.run();
                    }
                    window.changed();
                }
            },
            Kirigami.Action {
                text: "Cancel"
                icon.name: "dialog-cancel"
                onTriggered: confirmDialog.close()
            }
        ]
    }

    SizeDialog {
        id: sizeDialog

        host: window
    }

    RuleDialog {
        id: ruleDialog

        host: window
    }
}
