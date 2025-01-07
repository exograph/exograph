CREATE TABLE "persons" (
	"name" TEXT PRIMARY KEY,
	"age" INT NOT NULL,
	"address_street" TEXT,
	"address_zip" INT,
	"address_state" TEXT,
	"address_city" TEXT
);

CREATE TABLE "addresss" (
	"street" TEXT,
	"zip" INT,
	"state" TEXT,
	"city" TEXT,
	PRIMARY KEY ("street", "zip", "state", "city")
);

ALTER TABLE "persons" ADD CONSTRAINT "persons_address_fk" FOREIGN KEY ("address_city", "address_state", "address_street", "address_zip") REFERENCES "addresss" ("city", "state", "street", "zip");

