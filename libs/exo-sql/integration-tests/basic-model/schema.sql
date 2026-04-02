CREATE TABLE addresses (
    id SERIAL PRIMARY KEY,
    city TEXT NOT NULL
);

CREATE TABLE venues (
    id SERIAL PRIMARY KEY,
    name TEXT NOT NULL
);

CREATE TABLE artists (
    id SERIAL PRIMARY KEY,
    name TEXT NOT NULL,
    address_id INT REFERENCES addresses(id)
);

CREATE TABLE concerts (
    id SERIAL PRIMARY KEY,
    name TEXT NOT NULL,
    venue_id INT REFERENCES venues(id)
);

CREATE TABLE concert_artists (
    id SERIAL PRIMARY KEY,
    concert_id INT REFERENCES concerts(id),
    artist_id INT REFERENCES artists(id)
);
