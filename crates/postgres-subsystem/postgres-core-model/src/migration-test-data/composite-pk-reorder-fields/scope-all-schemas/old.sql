CREATE TABLE "people" (
	"name" TEXT PRIMARY KEY,
	"age" INT NOT NULL,
	"address_street" TEXT,
	"address_zip" INT,
	"address_state" TEXT,
	"address_city" TEXT
);

CREATE TABLE "addresses" (
	"street" TEXT,
	"zip" INT,
	"state" TEXT,
	"city" TEXT,
	PRIMARY KEY ("street", "zip", "state", "city")
);

ALTER TABLE "people" ADD CONSTRAINT "people_address_fk" FOREIGN KEY ("address_city", "address_state", "address_street", "address_zip") REFERENCES "addresses" ("city", "state", "street", "zip");

