CREATE EXTENSION IF NOT EXISTS "vector";

CREATE TABLE "documents" (
	"id" SERIAL PRIMARY KEY,
	"title" TEXT NOT NULL,
	"content" TEXT NOT NULL,
	"content_vector" Vector(3)
);

