CREATE SCHEMA "non_public";

CREATE TYPE "non_public"."priority" AS ENUM ('LOW', 'MEDIUM', 'HIGH');

CREATE TABLE "non_public"."todos" (
    "id" SERIAL PRIMARY KEY,
    "title" TEXT NOT NULL,
    "completed" BOOLEAN NOT NULL,
    "priority_with_default" "non_public"."priority" NOT NULL DEFAULT 'MEDIUM',
    "priority_nullable" "non_public"."priority",
    "priority" "non_public"."priority" NOT NULL 
);
