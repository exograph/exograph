-- ALTER TABLE "concerts" DROP COLUMN "updated_at";

-- ALTER TABLE "concerts" DROP COLUMN "modification_id";

DROP TRIGGER exograph_on_update_concerts on "concerts";

DROP FUNCTION exograph_update_concerts;

-- DROP EXTENSION "pgcrypto";

