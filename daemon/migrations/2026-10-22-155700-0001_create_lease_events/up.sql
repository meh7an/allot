CREATE TABLE lease_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    lease_id BIGINT NOT NULL REFERENCES leases (id) ON DELETE CASCADE,
    at_ms BIGINT NOT NULL,
    -- NULL for the event that records the lease's creation.
    from_state TEXT CHECK (from_state IN ('queued', 'active', 'released', 'expired')),
    to_state TEXT NOT NULL CHECK (to_state IN ('queued', 'active', 'released', 'expired')),
    reason TEXT NOT NULL
);

CREATE INDEX lease_events_lease_id ON lease_events (lease_id);
