CREATE TABLE "users" (
	"id" SERIAL PRIMARY KEY,
	"role" TEXT NOT NULL DEFAULT 'USER'::text,
	"verified" BOOLEAN NOT NULL DEFAULT true,
	"enabled" BOOLEAN NOT NULL
);

