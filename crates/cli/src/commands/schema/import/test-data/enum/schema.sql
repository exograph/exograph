CREATE TYPE "priority" AS ENUM ('LOW', 'MEDIUM', 'HIGH');

CREATE TABLE "todos" (
    "id" SERIAL PRIMARY KEY,
    "title" TEXT NOT NULL,
    "completed" BOOLEAN NOT NULL,
    "priority_with_default" "priority" NOT NULL DEFAULT 'MEDIUM',
    "priority_nullable" "priority",
    "priority" "priority" NOT NULL 
);
