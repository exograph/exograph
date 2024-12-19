CREATE TABLE "rsvps" (
	"id" SERIAL PRIMARY KEY,
	"email" TEXT NOT NULL,
	"event_id" INT NOT NULL
);

ALTER TABLE "rsvps" ADD CONSTRAINT "unique_constraint_rsvp_email_event_id" UNIQUE ("email");

