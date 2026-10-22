CREATE TABLE job_runs (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    -- History outlives the lease it ran under, so pruning leases must not delete runs.
    lease_id BIGINT REFERENCES leases (id) ON DELETE SET NULL,
    fingerprint TEXT NOT NULL,
    command TEXT NOT NULL,
    cwd TEXT NOT NULL,
    declared_cores FLOAT NOT NULL,
    declared_ram_mb BIGINT NOT NULL,
    declared_gpu_pct FLOAT NOT NULL,
    declared_vram_mb BIGINT NOT NULL,
    declared_est_secs BIGINT NOT NULL,
    peak_cores FLOAT NOT NULL,
    avg_cores FLOAT NOT NULL,
    peak_ram_mb BIGINT NOT NULL,
    avg_ram_mb BIGINT NOT NULL,
    peak_gpu_pct FLOAT NOT NULL,
    avg_gpu_pct FLOAT NOT NULL,
    peak_vram_mb BIGINT NOT NULL,
    avg_vram_mb BIGINT NOT NULL,
    started_at_ms BIGINT NOT NULL,
    ended_at_ms BIGINT NOT NULL,
    -- Load from processes outside the holder's session; only recorded for benchmark leases.
    noise_avg_cpu_pct FLOAT,
    noise_peak_cpu_pct FLOAT,
    noise_avg_gpu_pct FLOAT,
    noise_peak_gpu_pct FLOAT
);

CREATE INDEX job_runs_fingerprint ON job_runs (fingerprint, started_at_ms);
