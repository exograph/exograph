CREATE SCHEMA IF NOT EXISTS "non_public";

CREATE TABLE "non_public"."todos" (
	"id" SERIAL PRIMARY KEY,
	"title" TEXT NOT NULL
);

