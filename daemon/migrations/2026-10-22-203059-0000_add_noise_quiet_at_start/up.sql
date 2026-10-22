-- Whether the machine met the quiet limits when the benchmark started; NULL like the other
-- noise columns for runs that are not benchmarks.
ALTER TABLE job_runs ADD COLUMN noise_quiet_at_start BOOLEAN;
