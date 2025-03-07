CREATE TABLE "people" (
	"name" TEXT PRIMARY KEY,
	"age" INT NOT NULL,
	"address_street" TEXT,
	"address_city" TEXT,
	"address_state" TEXT,
	"address_zip" INT
);

CREATE TABLE "addresses" (
	"street" TEXT,
	"city" TEXT,
	"state" TEXT,
	"zip" INT,
	PRIMARY KEY ("street", "city", "state", "zip")
);

ALTER TABLE "people" ADD CONSTRAINT "people_address_fk" FOREIGN KEY ("address_city", "address_state", "address_street", "address_zip") REFERENCES "addresses" ("city", "state", "street", "zip");

