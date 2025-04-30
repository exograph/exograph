CREATE TYPE "non_public"."priority" AS ENUM ('LOW', 'MEDIUM', 'HIGH');

ALTER TABLE "non_public"."todos" ADD "priority" "non_public"."priority" NOT NULL;

ALTER TABLE "non_public"."todos" ADD "priority_with_default" "non_public"."priority" NOT NULL DEFAULT 'MEDIUM';

