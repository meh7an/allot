-- The cores a lease asked for and the fewest it may start with. `cores` stays what the lease
-- reserves, which is below `asked_cores` once a grant shrinks to fit. NULL for older leases.
ALTER TABLE leases ADD COLUMN asked_cores FLOAT;
ALTER TABLE leases ADD COLUMN min_cores FLOAT;
