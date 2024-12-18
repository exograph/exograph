CREATE TABLE "concerts" (
	"id" SERIAL PRIMARY KEY,
	"title" TEXT NOT NULL,
	"venue_id" INT
);

CREATE TABLE "venues" (
	"id" SERIAL PRIMARY KEY,
	"name" TEXT NOT NULL
);

ALTER TABLE "concerts" ADD CONSTRAINT "concerts_venue_id_fk" FOREIGN KEY ("venue_id") REFERENCES "venues";

