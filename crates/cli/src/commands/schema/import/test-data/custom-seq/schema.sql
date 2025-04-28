CREATE SEQUENCE IF NOT EXISTS "my_sequence";

CREATE TABLE "todos" (
        "id" INT PRIMARY KEY DEFAULT nextval('public.my_sequence'::regclass),
        "title" TEXT NOT NULL,
        "completed" BOOLEAN NOT NULL,
        "user_id" INT NOT NULL
);

CREATE TABLE "users" (
        "id" INT PRIMARY KEY DEFAULT nextval('public.my_sequence'::regclass),
        "name" TEXT NOT NULL
);

ALTER TABLE "todos" ADD CONSTRAINT "todos_user_fk" FOREIGN KEY ("user_id") REFERENCES "users";