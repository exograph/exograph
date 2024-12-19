CREATE TABLE "users" (
	"id" SERIAL PRIMARY KEY,
	"role" TEXT NOT NULL,
	"verified" BOOLEAN NOT NULL DEFAULT false,
	"enabled" BOOLEAN NOT NULL DEFAULT true
);

