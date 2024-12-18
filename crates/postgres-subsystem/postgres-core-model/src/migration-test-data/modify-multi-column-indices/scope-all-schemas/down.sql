DROP INDEX "title";

DROP INDEX "title-venue";

DROP INDEX "venue";

CREATE INDEX "concert_title_idx" ON "concerts" ("title");

CREATE INDEX "concert_venue_idx" ON "concerts" ("venue_id");

DROP INDEX "name";

CREATE INDEX "venue_name_idx" ON "venues" ("name");

