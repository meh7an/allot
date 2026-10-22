-- AUTOINCREMENT so a pruned lease ID is never handed out again to a different lease.
CREATE TABLE leases (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    session_id TEXT NOT NULL,
    holder_pid BIGINT NOT NULL,
    holder_pid_start_time BIGINT NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('normal', 'benchmark_exclusive')),
    state TEXT NOT NULL CHECK (state IN ('queued', 'active', 'released', 'expired')),
    class TEXT CHECK (class IN ('light', 'cpu_heavy', 'gpu_heavy', 'mixed')),
    cores FLOAT NOT NULL,
    ram_mb BIGINT NOT NULL,
    gpu_pct FLOAT NOT NULL,
    vram_mb BIGINT NOT NULL,
    est_secs BIGINT NOT NULL,
    label TEXT NOT NULL,
    requested_at_ms BIGINT NOT NULL,
    granted_at_ms BIGINT,
    ended_at_ms BIGINT,
    expires_at_ms BIGINT NOT NULL
);

CREATE INDEX leases_state ON leases (state);
CREATE INDEX leases_session_id ON leases (session_id);
