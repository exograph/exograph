CREATE TABLE issues (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT 'No description',
    status VARCHAR(50) NOT NULL DEFAULT 'pending'
);