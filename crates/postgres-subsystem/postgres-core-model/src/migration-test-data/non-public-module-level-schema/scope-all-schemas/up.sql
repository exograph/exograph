CREATE SCHEMA "info";

-- DROP TABLE "logs" CASCADE;

-- DROP TABLE "users" CASCADE;

CREATE TABLE "info"."logs" (
	"id" INT PRIMARY KEY,
	"level" TEXT,
	"message" TEXT NOT NULL,
	"owner_id" INT NOT NULL
);

CREATE TABLE "info"."users" (
	"id" INT PRIMARY KEY,
	"name" TEXT NOT NULL
);

ALTER TABLE "info"."logs" ADD CONSTRAINT "info_logs_owner_id_fk" FOREIGN KEY ("owner_id") REFERENCES "info"."users";

