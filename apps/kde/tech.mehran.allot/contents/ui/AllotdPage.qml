import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

import "../code/settings-view.mjs" as View

// One of allotd's pages: its banners, then its parts from the spec, built once GET /settings
// first answered (DESIGN.md sections 4 to 7). Each part is its own view, loaded by kind.
QQC2.ScrollView {
    id: page

    property var host: null
    property var entry: null
    property var pageModel: null
    property var sections: []
    property var banners: []
    property string bannerSignature: ""

    // Banners rebuild only when they change, so a focused banner button survives the refresh.
    function sync() {
        const model = page.host.model;
        if (model === null) {
            return;
        }
        if (page.pageModel === null) {
            const found = model.session.pages.find((candidate) =>
                candidate.spec.id === page.entry.id);
            if (found !== undefined) {
                page.pageModel = found;
                page.sections = View.pageSections(found);
            }
        }
        const banners = View.bannerViews(model.pageState(page.entry.id).banners);
        const signature = View.signature(banners);
        if (signature !== page.bannerSignature) {
            page.bannerSignature = signature;
            page.banners = banners;
        }
    }

    contentWidth: availableWidth

    Connections {
        target: page.host

        function onRevisionChanged() {
            page.sync();
        }
    }

    Component.onCompleted: page.sync()

    ColumnLayout {
        width: page.availableWidth
        spacing: Kirigami.Units.largeSpacing

        Kirigami.Heading {
            Layout.fillWidth: true
            Layout.margins: Kirigami.Units.largeSpacing
            Layout.bottomMargin: 0
            level: 1
            text: page.entry.label
        }

        QQC2.Label {
            Layout.fillWidth: true
            Layout.leftMargin: Kirigami.Units.largeSpacing
            Layout.rightMargin: Kirigami.Units.largeSpacing
            text: page.pageModel === null || page.pageModel.spec.intro === null ? ""
                : page.pageModel.spec.intro
            visible: text !== ""
            wrapMode: Text.Wrap
        }

        Repeater {
            model: page.banners

            Kirigami.InlineMessage {
                required property var modelData

                Layout.fillWidth: true
                Layout.leftMargin: Kirigami.Units.largeSpacing
                Layout.rightMargin: Kirigami.Units.largeSpacing
                visible: true
                text: modelData.text
                type: modelData.severity === "error" ? Kirigami.MessageType.Error
                    : modelData.severity === "warning" ? Kirigami.MessageType.Warning
                    : Kirigami.MessageType.Information
                // A banner has at most two buttons (settings-banners.js).
                actions: [
                    Kirigami.Action {
                        readonly property var action: modelData.actions[0] ?? null

                        visible: action !== null
                        text: action === null ? "" : action.label
                        onTriggered: page.host.model.act(action.id)
                    },
                    Kirigami.Action {
                        readonly property var action: modelData.actions[1] ?? null

                        visible: action !== null
                        text: action === null ? "" : action.label
                        onTriggered: page.host.model.act(action.id)
                    }
                ]
            }
        }

        QQC2.Label {
            Layout.fillWidth: true
            Layout.leftMargin: Kirigami.Units.largeSpacing
            text: View.LOADING_TEXT
            visible: {
                page.host.revision;
                return page.pageModel === null
                    && (page.host.model === null || page.host.model.session.status === "loading");
            }
        }

        Repeater {
            // By index: a model object that goes through a Repeater's modelData or a Loader's
            // initial properties arrives as a plain copy, without its getters and methods.
            model: page.sections.length

            ColumnLayout {
                id: slot

                required property int index
                readonly property var section: page.sections[index]

                // A loaded item's own Layout margins do nothing outside a layout, so they go here.
                Layout.fillWidth: true
                Layout.leftMargin: Kirigami.Units.largeSpacing
                Layout.rightMargin: Kirigami.Units.largeSpacing

                Loader {
                    Layout.fillWidth: true
                    sourceComponent: {
                        switch (slot.section.kind) {
                        case "settings":
                            return groupPart;
                        case "sizes":
                            return sizesPart;
                        case "rules":
                            return rulesPart;
                        case "reset":
                            return resetPart;
                        default:
                            return slot.section.id === "service_status" ? servicePart
                                : slot.section.id === "what_fits" ? fitsPart : filesPart;
                        }
                    }
                }

                Component {
                    id: groupPart

                    SettingsGroup {
                        host: page.host
                        section: slot.section
                        pageId: page.entry.id
                    }
                }

                Component {
                    id: sizesPart

                    SizeTable {
                        host: page.host
                        section: slot.section
                    }
                }

                Component {
                    id: rulesPart

                    RuleList {
                        host: page.host
                        section: slot.section
                    }
                }

                Component {
                    id: resetPart

                    ResetButtons {
                        host: page.host
                        section: slot.section
                        pageId: page.entry.id
                        pageLabel: page.entry.label
                    }
                }

                Component {
                    id: servicePart

                    ServiceStatus {
                        host: page.host
                    }
                }

                Component {
                    id: fitsPart

                    WhatFits {
                        host: page.host
                    }
                }

                Component {
                    id: filesPart

                    FilesGroup {
                        host: page.host
                    }
                }
            }
        }

        Item {
            Layout.preferredHeight: Kirigami.Units.largeSpacing
        }
    }
}
