CREATE SCHEMA "c";

CREATE SCHEMA "v";

CREATE TABLE "c"."concerts" (
	"id" SERIAL PRIMARY KEY,
	"title" TEXT NOT NULL,
	"venue_id" INT NOT NULL
);

CREATE TABLE "v"."venues" (
	"id" SERIAL PRIMARY KEY,
	"name" TEXT NOT NULL
);

ALTER TABLE "c"."concerts" ADD CONSTRAINT "c_concerts_venue_id_fk" FOREIGN KEY ("venue_id") REFERENCES "v"."venues";

CREATE INDEX "concert_title_idx" ON "c"."concerts" ("title");

CREATE INDEX "concert_venue_idx" ON "c"."concerts" ("venue_id");

CREATE INDEX "venue_name_idx" ON "v"."venues" ("name");

