CREATE SCHEMA "auth";

-- DROP TABLE "log"."users" CASCADE;

CREATE TABLE "auth"."users" (
	"id" INT PRIMARY KEY,
	"name" TEXT NOT NULL
);

-- DROP SCHEMA "log" CASCADE;
