CREATE TYPE "priority" AS ENUM ('LOW', 'MEDIUM', 'HIGH');

ALTER TABLE "todos" ADD "priority" "priority" NOT NULL;

ALTER TABLE "todos" ADD "priority_with_default" "priority" NOT NULL DEFAULT 'MEDIUM';

