ALTER TABLE "memberships" ADD "user_id" INT NOT NULL;

ALTER TABLE "memberships" ADD CONSTRAINT "unique_constraint_membership_user" UNIQUE ("user_id");

ALTER TABLE "memberships" ADD CONSTRAINT "memberships_user_id_fk" FOREIGN KEY ("user_id") REFERENCES "users";

