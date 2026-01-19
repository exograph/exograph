-- ALTER TABLE "concerts" DROP COLUMN "updated_at";

-- ALTER TABLE "concerts" DROP COLUMN "modification_id";

-- ALTER TABLE "concerts" DROP COLUMN "modification_id_v7";

DROP TRIGGER exograph_on_update_concerts on "concerts";

DROP FUNCTION exograph_update_concerts;
