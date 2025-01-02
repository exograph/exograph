CREATE TABLE "concerts" (
	"id" SERIAL PRIMARY KEY,
	"title" TEXT NOT NULL,
	"venue_id" INT NOT NULL
);

CREATE TABLE "venues" (
	"id" SERIAL PRIMARY KEY,
	"name" TEXT NOT NULL
);

ALTER TABLE "concerts" ADD CONSTRAINT "concerts_venue_fk" FOREIGN KEY ("venue_id") REFERENCES "venues";

CREATE INDEX "title" ON "concerts" ("title");

CREATE INDEX "title-venue" ON "concerts" ("title", "venue_id");

CREATE INDEX "venue" ON "concerts" ("venue_id");

CREATE INDEX "name" ON "venues" ("name");

