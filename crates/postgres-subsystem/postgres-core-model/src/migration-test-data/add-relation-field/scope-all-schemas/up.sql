ALTER TABLE "concerts" ADD "venue_id" INT NOT NULL;

ALTER TABLE "concerts" ADD CONSTRAINT "concerts_venue_id_fk" FOREIGN KEY ("venue_id") REFERENCES "venues";

