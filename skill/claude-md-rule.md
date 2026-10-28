## Shared Machine Resources

This machine is shared by several Claude sessions, coordinated by the Allot daemon.

- Before any heavy command (builds, test suites, benchmarks, training, rendering, or anything
  that keeps several cores or the GPU busy for more than ~30 s), follow the `resource-guard`
  skill: `acquire_lease`, run only while the lease is active, then `release_lease` right away.
- A tool call "Blocked by allot" means: do what the message says, then retry the same
  command. Never reword, wrap, or script a command to get past the hook.
- Timing or performance numbers need a `benchmark_exclusive` lease; tests and builds do not.
- When a lease is queued, tell the user in one line what is ahead and the estimated wait, then
  keep waiting; never ask whether to queue or wait. If it ends without starting (removed from
  the queue), don't run the command; say in one line that it was cancelled.
- When a note says the user stopped a running job from Allot, say so and ask whether to retry
  or skip; never retry on your own.
- Lease handling never needs the user's approval; it is not a state-changing command.
