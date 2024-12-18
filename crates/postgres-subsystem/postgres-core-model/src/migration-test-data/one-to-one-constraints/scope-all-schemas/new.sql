CREATE TABLE "memberships" (
	"id" SERIAL PRIMARY KEY,
	"user_id" INT NOT NULL
);

CREATE TABLE "users" (
	"id" SERIAL PRIMARY KEY,
	"name" TEXT NOT NULL
);

ALTER TABLE "memberships" ADD CONSTRAINT "memberships_user_id_fk" FOREIGN KEY ("user_id") REFERENCES "users";

ALTER TABLE "memberships" ADD CONSTRAINT "unique_constraint_membership_user" UNIQUE ("user_id");

