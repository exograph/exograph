ALTER TABLE "users" DROP CONSTRAINT "unique_constraint_user_email";

DROP TABLE "todos" CASCADE;

DROP TABLE "users" CASCADE;

CREATE TABLE "t_todos" (
	"id" SERIAL PRIMARY KEY,
	"title" TEXT NOT NULL,
	"completed" BOOLEAN NOT NULL,
	"user_id" INT NOT NULL
);

CREATE TABLE "t_users" (
	"id" SERIAL PRIMARY KEY,
	"email" TEXT NOT NULL
);

ALTER TABLE "t_todos" ADD CONSTRAINT "t_todos_user_fk" FOREIGN KEY ("user_id") REFERENCES "t_users";

ALTER TABLE "t_users" ADD CONSTRAINT "unique_constraint_user_email" UNIQUE ("email");

