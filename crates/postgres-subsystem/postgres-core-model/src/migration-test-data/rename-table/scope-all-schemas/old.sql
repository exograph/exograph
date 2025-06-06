CREATE TABLE "todos" (
	"id" SERIAL PRIMARY KEY,
	"title" TEXT NOT NULL,
	"completed" BOOLEAN NOT NULL,
	"user_id" INT NOT NULL
);

CREATE TABLE "users" (
	"id" SERIAL PRIMARY KEY,
	"email" TEXT NOT NULL
);

ALTER TABLE "todos" ADD CONSTRAINT "todos_user_fk" FOREIGN KEY ("user_id") REFERENCES "users";

ALTER TABLE "users" ADD CONSTRAINT "unique_constraint_user_email" UNIQUE ("email");

