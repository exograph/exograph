CREATE EXTENSION "vector";

CREATE TABLE "documents" (
	"id" SERIAL PRIMARY KEY,
	"title" TEXT NOT NULL,
	"content" TEXT NOT NULL,
	"content_vector" Vector(3)
);

CREATE INDEX "document_contentvector_idx" ON "documents" USING hnsw ("content_vector" vector_cosine_ops);

