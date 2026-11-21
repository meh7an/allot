import { quantityText } from "./setting-values.js";

/**
 * @import {
 *   ClassSize,
 *   CommandResult,
 *   CommandTest,
 *   HeavyRule,
 *   JobClass,
 *   SettingControl,
 * } from "../contract.g.mjs"
 * @import { RuleDraft, RuleListModel } from "./setting-values.js"
 * @import { Confirmation } from "./settings-text.js"
 */

// The Commands page's sentences: the rule list's status and rows, its confirmations and notices,
// and the lines of Test a command.

export const BUILT_IN_RULES_TEXT = "Built-in rules.";

export const YOUR_RULES_TEXT = "Your rules. They replace the built-in list.";

export const NO_RULES_TEXT =
  "No rules. No command counts as heavy, so the hook never asks for a lease.";

export const ANY_ARGUMENTS = "any arguments";

export const NO_COMMAND_TEXT = "No command to test in this line.";

export const DRAFT_NOT_READY_TEXT = "Enter a program to test this rule.";

/**
 * The status line above the list.
 *
 * @param {RuleListModel} rules
 * @returns {string}
 */
export function rulesStatus(rules) {
  return rules.canReset ? YOUR_RULES_TEXT : BUILT_IN_RULES_TEXT;
}

/**
 * A rule's arguments in its row: "build · test · run", or "any arguments".
 *
 * @param {HeavyRule} rule
 * @returns {string}
 */
export function ruleArguments(rule) {
  return rule.args.length === 0 ? ANY_ARGUMENTS : rule.args.join(" · ");
}

/**
 * A rule's row as a screen reader reads it: "Rule 1: cargo, build · test, CPU heavy".
 *
 * @param {number} index From 0.
 * @param {HeavyRule} rule
 * @param {(jobClass: JobClass) => string} classLabel
 * @returns {string}
 */
export function ruleRowName(index, rule, classLabel) {
  return `Rule ${index + 1}: ${rule.program}, ${ruleArguments(rule)}, ${classLabel(rule.class)}`;
}

/**
 * The accessible name of a rule's menu button.
 *
 * @param {number} index
 * @returns {string}
 */
export function ruleMenuName(index) {
  return `More options for rule ${index + 1}`;
}

/**
 * The notice after a delete, with Undo.
 *
 * @param {number} index
 * @returns {string}
 */
export function ruleDeletedText(index) {
  return `Rule ${index + 1} deleted.`;
}

/**
 * The rule dialog's title.
 *
 * @param {RuleDraft} draft
 * @returns {string}
 */
export function ruleDialogTitle(draft) {
  return draft.index === null ? "Add rule" : `Edit rule ${draft.index + 1}`;
}

/**
 * Use Built-in Rules' confirmation.
 *
 * @param {number} current The rules in the list now.
 * @param {number} builtIn The built-in rules.
 * @returns {Confirmation}
 */
export function useBuiltInConfirmation(current, builtIn) {
  const yours = current === 1 ? "Your 1 rule is" : `Your ${current} rules are`;
  const ones = builtIn === 1 ? "built-in one" : `${builtIn} built-in ones`;
  return {
    title: "Use the built-in rules?",
    body: `${yours} replaced by the ${ones}. This can't be undone.`,
    button: "Use Built-in Rules",
  };
}

/**
 * Names a class from the Classes table's spec, such as "CPU heavy".
 *
 * @param {SettingControl | null | undefined} classes The `classes` setting's control.
 * @returns {(jobClass: JobClass) => string}
 */
export function classLabels(classes) {
  const rows = classes?.kind === "size_table" ? classes.rows : [];
  return (jobClass) => rows.find((row) => row.name === jobClass)?.label ?? jobClass;
}

/**
 * One line per simple command of the tested line, and one more naming the combined lease when
 * more than one command is heavy.
 *
 * @param {CommandTest} test allotd's answer.
 * @param {HeavyRule[]} rules The rules it was tested against, for the matched rule's text.
 * @param {(jobClass: JobClass) => string} classLabel
 * @param {(value: number) => string} number
 * @returns {string[]}
 */
export function testLines(test, rules, classLabel, number) {
  if (test.commands.length === 0) {
    return [NO_COMMAND_TEXT];
  }
  const lines = test.commands.map((result) => {
    switch (result.kind) {
      case "heavy":
        return heavyLine(result, rules, classLabel, number);
      case "print_only":
        return `Print-only: ${result.flag ?? ""} only prints and exits, so no lease is needed.`;
      case "not_heavy":
        return `Not heavy: no rule matches ${result.command}.`;
    }
  });
  const heavy = test.commands.filter((result) => result.kind === "heavy").length;
  if (heavy > 1 && test.lease !== null) {
    lines.push(`The whole line needs one ${leaseText(test.lease, classLabel, number)}.`);
  }
  return lines;
}

/**
 * The rules that matched a command of the tested line, from 0, for the "Matches" mark.
 *
 * @param {CommandTest | null} test
 * @returns {number[]}
 */
export function matchedRules(test) {
  /** @type {number[]} */
  const matched = [];
  for (const result of test?.commands ?? []) {
    const index = result.rule_index;
    if (result.kind === "heavy" && index !== null && !matched.includes(index)) {
      matched.push(index);
    }
  }
  return matched;
}

/**
 * @param {CommandResult} result
 * @param {HeavyRule[]} rules
 * @param {(jobClass: JobClass) => string} classLabel
 * @param {(value: number) => string} number
 * @returns {string}
 */
function heavyLine(result, rules, classLabel, number) {
  const needs =
    result.size === null
      ? "Heavy."
      : `Heavy: needs a ${leaseText(result.size, classLabel, number)}.`;
  const index = result.rule_index;
  const rule = index === null ? undefined : rules[index];
  if (index === null || rule === undefined) {
    return needs;
  }
  const args = rule.args.length === 0 ? ANY_ARGUMENTS : alternatives(rule.args);
  return `${needs} Rule ${index + 1}: ${rule.program} with ${args}.`;
}

/**
 * "CPU heavy lease (8 cores, 4 GB memory)"; the GPU parts only when the class asks for them.
 *
 * @param {ClassSize} size
 * @param {(jobClass: JobClass) => string} classLabel
 * @param {(value: number) => string} number
 * @returns {string}
 */
function leaseText(size, classLabel, number) {
  const profile = size.profile;
  const parts = [
    quantityText(profile.cores, "cores", number),
    `${quantityText(profile.ram_mb, "megabytes", number)} memory`,
  ];
  if (profile.gpu_pct > 0) {
    parts.push(`${quantityText(profile.gpu_pct, "percent", number)} of the GPU`);
  }
  if (profile.vram_mb > 0) {
    parts.push(`${quantityText(profile.vram_mb, "megabytes", number)} video memory`);
  }
  return `${classLabel(size.class)} lease (${parts.join(", ")})`;
}

/**
 * "build, test, run or bench".
 *
 * @param {string[]} parts
 * @returns {string}
 */
function alternatives(parts) {
  if (parts.length < 2) {
    return parts.join("");
  }
  return `${parts.slice(0, -1).join(", ")} or ${parts[parts.length - 1]}`;
}
