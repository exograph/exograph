DROP INDEX "concert_title_idx";

DROP INDEX "concert_venue_idx";

CREATE INDEX "title" ON "concerts" ("title");

CREATE INDEX "title-venue" ON "concerts" ("title", "venue_id");

CREATE INDEX "venue" ON "concerts" ("venue_id");

DROP INDEX "venue_name_idx";

CREATE INDEX "name" ON "venues" ("name");

