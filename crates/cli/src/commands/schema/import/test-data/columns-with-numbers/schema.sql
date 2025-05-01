CREATE TABLE "metrics" (
        "id" SERIAL PRIMARY KEY,
        "name" TEXT NOT NULL,
        "min_30d_price" REAL NOT NULL,
        "max30d_price" REAL NOT NULL
);