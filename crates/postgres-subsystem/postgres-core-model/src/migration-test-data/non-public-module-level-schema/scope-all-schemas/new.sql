CREATE SCHEMA IF NOT EXISTS "info";

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

ALTER TABLE "info"."logs" ADD CONSTRAINT "info_logs_owner_fk" FOREIGN KEY ("owner_id") REFERENCES "info"."users";

