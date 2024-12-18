-- ALTER TABLE "documents" DROP COLUMN "content_vector";

ALTER TABLE "documents" ADD "content_vector" Vector(4);

