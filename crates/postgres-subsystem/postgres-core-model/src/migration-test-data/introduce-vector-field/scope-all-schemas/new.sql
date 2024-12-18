CREATE EXTENSION "vector";

CREATE TABLE "documents" (
	"id" SERIAL PRIMARY KEY,
	"content" TEXT NOT NULL,
	"content_vector" Vector(1536)
);

