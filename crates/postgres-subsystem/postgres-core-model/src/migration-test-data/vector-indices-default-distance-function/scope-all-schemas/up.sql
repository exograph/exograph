CREATE INDEX "document_contentvector_idx" ON "documents" USING hnsw ("content_vector" vector_cosine_ops);

