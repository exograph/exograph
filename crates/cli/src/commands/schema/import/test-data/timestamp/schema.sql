CREATE TABLE issues (
    id SERIAL PRIMARY KEY,
    name VARCHAR(200) NOT NULL,
    due_date DATE NOT NULL DEFAULT CURRENT_DATE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
);