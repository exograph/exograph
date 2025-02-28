ALTER TABLE "users" DROP CONSTRAINT "unique_constraint_user_email";

ALTER TABLE "todos" RENAME TO "t_todos";

DROP TABLE "users" CASCADE;

CREATE TABLE "t_users" (
	"id" SERIAL PRIMARY KEY,
	"email" TEXT NOT NULL
);

ALTER TABLE "t_users" ADD CONSTRAINT "unique_constraint_user_email" UNIQUE ("email");

