/** allotd's port when the config does not set one, as in `DaemonConfig::default`. */
export const DEFAULT_PORT = 47823;

const MAX_PORT = 65535;

/**
 * The file allotd writes its bearer token to on first start.
 *
 * @param {string} root `~/.allot`.
 * @returns {string}
 */
export function tokenPath(root) {
  return `${root}/token`;
}

/**
 * Finds the port as either `port` under `[daemon]` or a top-level `daemon.port`. Not a TOML
 * parser: allotd validates the whole file and refuses to start on a bad one, so anything this
 * cannot read falls back to the default.
 *
 * @param {string} toml
 * @returns {number}
 */
export function portFromToml(toml) {
  let table = "";
  for (const rawLine of toml.split(/\r?\n/)) {
    const line = stripComment(rawLine).trim();
    if (line === "") {
      continue;
    }
    if (line.startsWith("[") && line.endsWith("]")) {
      table = line.slice(1, -1).trim();
      continue;
    }
    const equals = line.indexOf("=");
    if (equals < 0) {
      continue;
    }
    const key = line.slice(0, equals).trim();
    if ((table === "daemon" && key === "port") || (table === "" && key === "daemon.port")) {
      return parsePort(line.slice(equals + 1));
    }
  }
  return DEFAULT_PORT;
}

/**
 * The port is an integer, so a `#` always starts a comment.
 *
 * @param {string} line
 * @returns {string}
 */
function stripComment(line) {
  const hash = line.indexOf("#");
  return hash < 0 ? line : line.slice(0, hash);
}

/**
 * TOML allows underscores between digits, as in the 47_823 allotd's own default uses.
 *
 * @param {string} value
 * @returns {number}
 */
function parsePort(value) {
  const digits = value.trim().replace(/_/g, "");
  if (!/^\d+$/.test(digits)) {
    return DEFAULT_PORT;
  }
  const port = Number(digits);
  return port > 0 && port <= MAX_PORT ? port : DEFAULT_PORT;
}
