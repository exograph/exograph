CREATE TABLE issues (
    id SERIAL PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT 'No description',
    status VARCHAR(50) NOT NULL DEFAULT 'pending'
);