-- The cores a request declared before history lowered them. A lowered lease counts at them
-- for its first seconds after the grant. NULL when history did not lower the cores.
ALTER TABLE leases ADD COLUMN declared_cores FLOAT;
