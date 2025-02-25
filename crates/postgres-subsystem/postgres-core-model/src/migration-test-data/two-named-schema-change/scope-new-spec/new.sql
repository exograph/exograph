CREATE SCHEMA IF NOT EXISTS "auth";

CREATE TABLE "auth"."users" (
	"id" INT PRIMARY KEY,
	"name" TEXT NOT NULL
);

