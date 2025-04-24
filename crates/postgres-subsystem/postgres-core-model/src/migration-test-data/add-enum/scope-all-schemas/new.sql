CREATE TYPE "priority" AS ENUM ('LOW', 'MEDIUM', 'HIGH');

CREATE TABLE "todos" (
	"id" SERIAL PRIMARY KEY,
	"title" TEXT NOT NULL,
	"priority" "priority" NOT NULL
);

