-- ALTER TABLE "memberships" DROP COLUMN "user_id";

ALTER TABLE "memberships" DROP CONSTRAINT IF EXISTS "unique_constraint_membership_user";

