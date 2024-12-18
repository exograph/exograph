CREATE EXTENSION "pgcrypto";

CREATE TABLE "concerts" (
	"id" SERIAL PRIMARY KEY,
	"title" TEXT NOT NULL,
	"updated_at" TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT now(),
	"modification_id" uuid NOT NULL DEFAULT gen_random_uuid()
);

CREATE FUNCTION exograph_update_concerts() RETURNS TRIGGER AS $$ BEGIN NEW.updated_at = now(); NEW.modification_id = gen_random_uuid(); RETURN NEW; END; $$ language 'plpgsql';

CREATE TRIGGER exograph_on_update_concerts BEFORE UPDATE ON concerts FOR EACH ROW EXECUTE FUNCTION exograph_update_concerts();

