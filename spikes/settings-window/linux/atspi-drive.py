#!/usr/bin/env python3
"""Drives a Settings window through AT-SPI, as a screen reader would, for the M0.3 smoke runs.

  dump WINDOW               the window's tree: role | name | states | children | value
  set WINDOW NAME VALUE     sets the number control named NAME, or the first after that name
  press WINDOW NAME         runs the first action of the control named NAME, or of the first after
                            that name (buttons, switches, check boxes, page tabs, list items)
  value WINDOW NAME         prints the number of the control named NAME
  actions WINDOW NAME       lists the actions of the node named NAME
  windows                   every application and its top-level windows

WINDOW is a window title. Exits 1 when the window or the control is not found.
"""

import sys
import warnings

import gi

gi.require_version("Atspi", "2.0")
from gi.repository import Atspi  # noqa: E402

STATES = {
    Atspi.StateType.ACTIVE: "active",
    Atspi.StateType.CHECKED: "checked",
    Atspi.StateType.FOCUSED: "focused",
    Atspi.StateType.SENSITIVE: "sensitive",
    Atspi.StateType.SHOWING: "showing",
}
ACTION_ROLES = {
    Atspi.Role.PUSH_BUTTON,
    Atspi.Role.TOGGLE_BUTTON,
    Atspi.Role.CHECK_BOX,
    Atspi.Role.SWITCH,
    Atspi.Role.RADIO_BUTTON,
    Atspi.Role.PAGE_TAB,
    Atspi.Role.LIST_ITEM,
}

PRESS_ACTIONS = {"press", "toggle", "click", "activate"}


def children(node):
    for index in range(node.get_child_count()):
        try:
            child = node.get_child_at_index(index)
        except Exception as error:  # noqa: BLE001 - a dead node must not end the walk
            print(f"# child {index} of {node.get_role_name()} {node.get_name()!r}: {error}")
            continue
        if child is not None:
            yield child


def walk(node, depth=0):
    yield node, depth
    for child in children(node):
        yield from walk(child, depth + 1)


def windows():
    desktop = Atspi.get_desktop(0)
    for app in children(desktop):
        for window in children(app):
            yield app, window


def window_named(title):
    for _app, window in windows():
        if window.get_name() == title:
            return window
    sys.exit(f"no window named {title!r}")


def number(node):
    value = node.get_value_iface()
    return None if value is None else value.get_current_value()


def control(window, name, wanted):
    """A node named name that wanted() accepts, else the first one after such a name."""
    nodes = [node for node, _depth in walk(window)]
    for node in nodes:
        if node.get_name() == name and wanted(node):
            return node
    for index, node in enumerate(nodes):
        if node.get_name() == name:
            for candidate in nodes[index:]:
                if wanted(candidate):
                    return candidate
    sys.exit(f"no control for {name!r}")


def has_value(node):
    return node.get_value_iface() is not None


def has_action(node):
    action = node.get_action_iface()
    return node.get_role() in ACTION_ROLES and action is not None and action.get_n_actions() > 0


def action_names(node):
    action = node.get_action_iface()
    count = 0 if action is None else action.get_n_actions()
    # get_action_name is deprecated, but its replacement clashes with the accessible's get_name.
    with warnings.catch_warnings():
        warnings.simplefilter("ignore", DeprecationWarning)
        return [action.get_action_name(index) for index in range(count)]


def main_action(node):
    """Qt lists "Set Focus" first on some controls, so the action that presses comes first."""
    names = [name.lower() for name in action_names(node)]
    for index, name in enumerate(names):
        if name in PRESS_ACTIONS:
            return index
    return 0


def dump(title):
    for node, depth in walk(window_named(title)):
        states = node.get_state_set()
        shown = ",".join(word for state, word in STATES.items() if states.contains(state))
        extra = number(node)
        print(
            f"{'  ' * depth}{node.get_role_name()} | {node.get_name()!r} | {shown}"
            + f" | {node.get_child_count()} children"
            + ("" if extra is None else f" | {extra}")
        )


def main(argv):
    command = argv[1]
    if command == "windows":
        for app, window in windows():
            print(f"{app.get_name()} | {window.get_role_name()} | {window.get_name()!r}")
    elif command == "dump":
        dump(argv[2])
    elif command == "set":
        node = control(window_named(argv[2]), argv[3], has_value)
        node.get_value_iface().set_current_value(float(argv[4]))
        print(f"set {argv[3]} = {number(node)}")
    elif command == "press":
        node = control(window_named(argv[2]), argv[3], has_action)
        role, name = node.get_role_name(), node.get_name()
        # The node may be gone once its action ran, as when OK closes a dialog.
        node.get_action_iface().do_action(main_action(node))
        print(f"pressed {role} {name!r}")
    elif command == "actions":
        print(action_names(control(window_named(argv[2]), argv[3], lambda candidate: True)))
    elif command == "value":
        print(number(control(window_named(argv[2]), argv[3], has_value)))
    else:
        sys.exit(__doc__)


main(sys.argv)
