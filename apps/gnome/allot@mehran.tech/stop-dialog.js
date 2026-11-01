import Clutter from "gi://Clutter";

import * as Dialog from "resource:///org/gnome/shell/ui/dialog.js";
import * as ModalDialog from "resource:///org/gnome/shell/ui/modalDialog.js";

import { STOP_CONFIRM_TITLE } from "./lib/service-menu.js";

/**
 * Asks before stopping allotd while jobs run or wait.
 *
 * @param {string} description
 * @param {() => void} onStop Stop Service was pressed.
 */
export function confirmStop(description, onStop) {
  const dialog = new ModalDialog.ModalDialog();
  dialog.contentLayout.add_child(
    new Dialog.MessageDialogContent({ title: STOP_CONFIRM_TITLE, description }),
  );
  dialog.addButton({
    label: "Cancel",
    action: () => dialog.close(),
    key: Clutter.KEY_Escape,
  });
  dialog.addButton({
    label: "Stop Service",
    action: () => {
      dialog.close();
      onStop();
    },
  });
  dialog.open();
}
