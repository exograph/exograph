ALTER TABLE "rsvps" ADD CONSTRAINT "unique_constraint_rsvp_email_event_id" UNIQUE ("email", "event_id");

