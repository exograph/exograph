CREATE SCHEMA IF NOT EXISTS "auth";

-- DROP TABLE "users" CASCADE;

CREATE TABLE "auth"."users" (
	"id" INT PRIMARY KEY,
	"name" TEXT NOT NULL
);

