CREATE SCHEMA "log";

-- DROP TABLE "auth"."users" CASCADE;

CREATE TABLE "log"."users" (
	"id" INT PRIMARY KEY,
	"name" TEXT NOT NULL
);

-- DROP SCHEMA "auth" CASCADE;

