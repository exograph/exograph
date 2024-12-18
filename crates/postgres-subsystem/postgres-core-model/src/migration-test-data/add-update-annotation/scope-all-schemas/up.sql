CREATE FUNCTION exograph_update_concerts() RETURNS TRIGGER AS $$ BEGIN NEW.updated_at = now(); RETURN NEW; END; $$ language 'plpgsql';

CREATE TRIGGER exograph_on_update_concerts BEFORE UPDATE ON concerts FOR EACH ROW EXECUTE FUNCTION exograph_update_concerts();

