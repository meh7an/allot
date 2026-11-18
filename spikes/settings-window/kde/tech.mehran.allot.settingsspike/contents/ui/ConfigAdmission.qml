import QtQuick
import org.kde.kcmutils as KCM
import org.kde.plasma.plasmoid

import "../code/settings-client.mjs" as Client

// Host A: allotd's settings as a page of the widget's config dialog. Edits wait for Apply or OK,
// which the dialog only enables through configurationChanged() (Plasma 6.0) or unsavedChanges
// (later versions), since allotd's values are not main.xml keys.
KCM.SimpleKCM {
    id: page

    // Every main.xml key arrives as a cfg_ property; Apply writes them back to the config.
    property bool cfg_showCpu
    property bool cfg_sendThroughWidget
    property string cfg_pendingWrite

    property bool unsavedChanges: false
    property var settings: null
    property var edits: ({})

    signal configurationChanged()

    function load() {
        io.client.load().then((outcome) => {
            io.log("load", { status: outcome.status, detail: outcome.detail });
            page.showOutcome(outcome);
        });
    }

    function showOutcome(outcome) {
        if (outcome.settings !== null) {
            page.settings = outcome.settings;
            form.show(outcome.settings);
        } else if (outcome.status === "invalid") {
            form.notify(outcome.errors.map((error) => error.message).join("; "));
            form.show(page.settings);
        } else {
            form.showDown(outcome.detail);
        }
        if (outcome.status === "conflict") {
            form.notify("Settings changed outside this window; showing the new values.");
        }
    }

    // Called by the dialog on Apply and OK, before it copies the cfg_ properties back. OK closes
    // the dialog right after, so a direct write may outlive this page.
    function saveConfig() {
        if (page.settings === null) {
            return;
        }
        const set = Client.changedValues(page.settings, page.edits);
        page.edits = {};
        page.unsavedChanges = false;
        if (Object.keys(set).length === 0) {
            return;
        }
        if (page.cfg_sendThroughWidget) {
            page.cfg_pendingWrite = JSON.stringify({
                revision: page.settings.revision,
                set: set,
                nonce: Date.now(),
            });
            io.log("handed-to-widget", { set: set });
            return;
        }
        io.log("save-start", { set: set });
        io.client.apply(page.settings.revision, set, []).then((outcome) => {
            io.log("save-done", { status: outcome.status });
            page.showOutcome(outcome);
        });
    }

    Component.onCompleted: {
        io.log("page-created", {
            plasmoidTitle: Plasmoid.title,
            hasRootItem: Plasmoid.rootItem !== undefined,
        });
        page.load();
    }
    Component.onDestruction: io.log("page-destroyed")

    SpikeIo {
        id: io

        where: "dialog-admission"
    }

    AdmissionForm {
        id: form

        onEdited: (key, value) => {
            page.edits[key] = value;
            page.unsavedChanges = true;
            page.configurationChanged();
            io.log("edit", { key: key, value: value });
        }
        onRetry: page.load()
    }
}
