-- DROP TABLE "c"."concerts" CASCADE;

-- DROP TABLE "v"."venues" CASCADE;

CREATE TABLE "concerts" (
	"id" SERIAL PRIMARY KEY,
	"title" TEXT NOT NULL,
	"venue_id" INT NOT NULL
);

CREATE TABLE "venues" (
	"id" SERIAL PRIMARY KEY,
	"name" TEXT NOT NULL
);

-- DROP SCHEMA IF EXISTS "c" CASCADE;

-- DROP SCHEMA IF EXISTS "v" CASCADE;

ALTER TABLE "concerts" ADD CONSTRAINT "concerts_venue_fk" FOREIGN KEY ("venue_id") REFERENCES "venues";

