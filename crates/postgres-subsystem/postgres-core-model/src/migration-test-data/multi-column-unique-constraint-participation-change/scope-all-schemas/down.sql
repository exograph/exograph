ALTER TABLE "rsvps" DROP CONSTRAINT "unique_constraint_rsvp_email_event_id";

ALTER TABLE "rsvps" ADD CONSTRAINT "unique_constraint_rsvp_email_event_id" UNIQUE ("email");

