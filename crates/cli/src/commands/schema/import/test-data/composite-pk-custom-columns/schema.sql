CREATE TABLE "people" (
	"first_name" TEXT,
	"last_name" TEXT,
	"age" INT NOT NULL,
	"astreet" TEXT,
	"acity" TEXT,
	"astate" TEXT,
	"azip" INT,
	PRIMARY KEY ("first_name", "last_name")
);

CREATE TABLE "addresses" (
	"street" TEXT,
	"city" TEXT,
	"state" TEXT,
	"zip" INT,
	"info" TEXT,
	PRIMARY KEY ("street", "city", "state", "zip")
);

ALTER TABLE "people" ADD CONSTRAINT "people_address_fk" FOREIGN KEY ("acity", "astate", "astreet", "azip") REFERENCES "addresses" ("city", "state", "street", "zip");
