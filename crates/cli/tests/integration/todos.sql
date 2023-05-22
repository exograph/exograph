CREATE TABLE "todos" (
	"id" SERIAL PRIMARY KEY,
	"title" TEXT NOT NULL,
	"completed" BOOLEAN NOT NULL
);

CREATE INDEX ON "todos" ("title");

CREATE INDEX ON "todos" ("completed");

