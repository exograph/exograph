-- Pk columns are not named `id`, but `<singularized_table_name>_id`)
-- Some table name (venue) uses the singular form

CREATE TABLE concerts (
    concert_id character varying(10) NOT NULL PRIMARY KEY,
    concert_name character varying(20) NOT NULL,
    venue_id smallint NOT NULL
);

CREATE TABLE venue (
    venue_id smallint NOT NULL PRIMARY KEY,
    venue_name character varying(20) NOT NULL
);

ALTER TABLE ONLY concerts
    ADD CONSTRAINT fk_concerts_venue FOREIGN KEY (venue_id) REFERENCES venue;