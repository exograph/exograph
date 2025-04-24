CREATE TYPE "priority" AS ENUM ('LOW', 'MEDIUM', 'HIGH');

ALTER TABLE "todos" ADD "priority" "priority" NOT NULL;

