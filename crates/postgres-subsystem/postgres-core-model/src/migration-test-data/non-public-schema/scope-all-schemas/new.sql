CREATE SCHEMA "auth";

CREATE TABLE "logs" (
	"id" INT PRIMARY KEY,
	"level" TEXT,
	"message" TEXT NOT NULL,
	"owner_id" INT NOT NULL
);

CREATE TABLE "auth"."users" (
	"id" INT PRIMARY KEY,
	"name" TEXT NOT NULL
);

ALTER TABLE "logs" ADD CONSTRAINT "logs_owner_id_fk" FOREIGN KEY ("owner_id") REFERENCES "auth"."users";

