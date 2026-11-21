// allotd that runs but is not on the session bus: started inside a sandbox, or by a session
// whose bus has since gone. The panel cannot reach it, yet starting another one would only fight
// over the instance lock, so the panel looks for it in the files before it calls the service
// stopped.

/** The file allotd locks for its single instance and writes its process id into. */
export const LOCK_FILE = "allotd.lock";

/** The process name the kernel reports for allotd. */
export const DAEMON_COMM = "allotd";

export const OFF_BUS_DETAIL = "the service runs without the session bus";

/**
 * The process id in the lock file's text, or `null` when there is none: an allotd from before it
 * wrote its id leaves the file empty.
 *
 * @param {string | null} text
 * @returns {number | null}
 */
export function lockPid(text) {
  const match = /^\s*(\d+)\s*$/.exec(text ?? "");
  const pid = match?.[1] === undefined ? 0 : Number(match[1]);
  return Number.isSafeInteger(pid) && pid > 0 ? pid : null;
}

/**
 * Whether an allotd process is alive, from file reads only: the lock file names the process, and
 * `/proc` says whether that process is still allotd, since an id may be reused after a crash.
 *
 * @param {string} root `~/.allot`.
 * @param {(path: string) => Promise<string | null>} readText A file's text, or `null` when it
 *   cannot be read.
 * @returns {Promise<boolean>}
 */
export async function allotdAliveOffBus(root, readText) {
  const pid = lockPid(await readText(`${root}/${LOCK_FILE}`));
  if (pid === null) {
    return false;
  }
  const comm = await readText(`/proc/${String(pid)}/comm`);
  return comm !== null && comm.trim() === DAEMON_COMM;
}
