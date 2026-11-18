import QtQuick
import org.kde.plasma.plasma5support as P5Support

import "../code/settings-client.mjs" as Client

// The settings client for one host (the widget, a dialog page or the window). QML can neither read
// files nor time out a request by itself, so the token comes through the executable data engine,
// and XMLHttpRequest is sent here: it calls back only when send() runs in QML code.
Item {
    id: io

    // Which host logs, so the session log tells the three apart.
    required property string where

    readonly property var client: new Client.SettingsClient({
        send: io.send,
        readToken: io.readToken,
    })

    // Commands waiting for the data engine, by source name.
    property var waiting: ({})

    function log(event, fields) {
        const line = Object.assign({ event: event, where: io.where }, fields || {});
        console.log("SettingsSpike " + JSON.stringify(line));
    }

    function readToken() {
        return io.run("cat \"$HOME/.allot/token\"").then((output) =>
            output.exitCode === 0 ? output.stdout : null);
    }

    function run(command) {
        return new Promise((resolve) => {
            const list = io.waiting[command];
            if (list !== undefined) {
                list.push(resolve);
                return;
            }
            io.waiting[command] = [resolve];
            executable.connectSource(command);
        });
    }

    function send(request) {
        return new Promise((resolve, reject) => {
            const xhr = new XMLHttpRequest();
            xhr.onreadystatechange = () => {
                if (xhr.readyState !== XMLHttpRequest.DONE) {
                    return;
                }
                if (xhr.status === 0) {
                    reject(new Error("no answer"));
                } else {
                    resolve({ status: xhr.status, body: xhr.responseText });
                }
            };
            xhr.open(request.method, request.url);
            xhr.setRequestHeader("Authorization", "Bearer " + request.token);
            if (request.body !== null) {
                xhr.setRequestHeader("Content-Type", "application/json");
                xhr.send(request.body);
            } else {
                xhr.send();
            }
        });
    }

    P5Support.DataSource {
        id: executable

        engine: "executable"
        connectedSources: []
        onNewData: (source, data) => {
            // The engine keeps a source and its answer until it is disconnected, so the same
            // command can run again only after this.
            disconnectSource(source);
            const list = io.waiting[source] || [];
            delete io.waiting[source];
            const output = { exitCode: data["exit code"], stdout: data.stdout };
            list.forEach((resolve) => resolve(output));
        }
    }
}
