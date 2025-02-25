CREATE EXTENSION IF NOT EXISTS "vector";

ALTER TABLE "documents" ADD "content_vector" Vector(1536);

