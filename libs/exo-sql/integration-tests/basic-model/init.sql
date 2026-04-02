INSERT INTO addresses (id, city) VALUES (1, 'New York'), (2, 'Los Angeles');

INSERT INTO venues (id, name) VALUES (1, 'Madison Square Garden'), (2, 'Hollywood Bowl');

INSERT INTO artists (id, name, address_id) VALUES (1, 'Artist1', 1), (2, 'Artist2', 2);

INSERT INTO concerts (id, name, venue_id) VALUES
    (1, 'Concert1', 1),
    (2, 'Concert2', 2),
    (3, 'Concert3', 1);

INSERT INTO concert_artists (id, concert_id, artist_id) VALUES
    (1, 1, 1),
    (2, 1, 2),
    (3, 2, 1);
