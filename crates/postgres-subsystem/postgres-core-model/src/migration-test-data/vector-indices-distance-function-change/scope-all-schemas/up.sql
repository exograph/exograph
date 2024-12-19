DROP INDEX "document_contentvector_idx";

CREATE INDEX "document_contentvector_idx" ON "documents" USING hnsw ("content_vector" vector_l2_ops);

