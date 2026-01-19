---
sidebar_position: 3
---

# Customizing Schema

Exograph infers the mapping of a type to a table, which is often good enough. However, if you need to customize the mapping, Exograph provides a few annotations. This section will examine the default Exograph mapping and how to customize it.

## Type-level customization

For a given type, Exograph infers:

- The associated table name
- The plural version of the type name (used to form [query](operations/queries.md) and [mutation](operations/mutations.md) names).

Exograph sets the default access control rules to disallow anyone executing associated queries and mutations. You can customize this rule using the `@access` annotation, but we will defer to the [Access Control](./access-control.md) section.

### Table name

By default, Exograph uses the **pluralized and snake_cased** type name as the table name. For example, Exograph will map the `Todo` type to the `todos` table, while it will map `AuthUser` to the `auth_users` table. There is an exception to this rule if a type carries the [`@plural`](#pluralization) annotation, as we'll see next.

You can customize the table's name using the `@table` annotation, whose sole single argument specifies the table's name. For example, if you want to map the `Todo` to the `t_todo` table (typically to match your organization's naming conventions), you can use the `@table` annotation as follows:

```exo
// highlight-next-line
@table("t_todo")
type Todo {
  ...
}
```

While the `@table` annotation affects the table name associated with the type, it leaves the query and mutation names unchanged. So, in the above example, the [query to get multiple todos](operations/queries.md#collection-query) will still be called `todos`, which is most likely what you want. However, this doesn't work out well in a few cases. For example, consider the following setup:

```exo
@table("people")
type Person {
  ...
}
```

Exograph will map the `Person` to the `people` table here. However, the query to get multiple people will still be `persons`, which is not ideal. To change the query and mutation names along with the table name, you can use the `@plural` annotation.

### Table schema

By default, Exograph assumes that a table will be in the `public` schema. You can customize its schema by specifying the `schema` attribute of the `@table` annotation. For example, to map the table `User` to be in the `auth` schema, you can use the `@table` annotation as follows:

```exo
@table(schema="auth")
type User {
  ...
}
```

The `User` type will be mapped to the `auth` schema, and the table name will be `users` (following the default naming convention). However, if you want to customize the table name as well, you can use the `@table` annotation as follows:

```exo
@table(schema="auth", name="t_users")
type User {
  ...
}
```

The `User` type will be mapped to the `auth` schema, and the table name will be `t_users`.

A common use case is to set the schema of all tables to a specific schema. You can achieve this by setting the `schema` attribute of the `@postgres` annotation. For example, to set the schema of all tables to `entertainment`, you can use the `@postgres` annotation as follows:

```exo
@postgres(schema="entertainment")
module ConcertModule {
  type Concert {
    ...
    venue: Venue
  }

  type Venue {
    ...
    concerts: Set<Concert>?
  }
}
```

You may still override the schema of a specific table using the `@table` annotation and the `schema` attribute.

### Pluralization

By default, Exograph will use a simple algorithm to pluralize the name of the type. However, it doesn't work well for names with irregular pluralization. For example, Exograph will pluralize `person` to `persons`, but you will likely want to name it `people`. You can control the plural form using the `@plural` annotation:

```exo
@plural("people")
type Person {
  ...
}
```

The `@plural` annotation's argument specifies the plural form of the type name. Exograph will use the provided plural form instead of its algorithm when this annotation is present. The plural name affects the table, query, and mutation names. For example, the `@plural` annotation above will map the `Person` type to the `people` table, the query to get multiple people will be `people`, and the mutation to delete multiple people will be `deletePeople`.

If both `@plural` and `@table` annotations are present, Exograph will use the argument provided to the `@table` annotation to name the table and the argument supplied to the `@plural` annotation to name the queries and mutations.

:::tip
Use the `@plural` annotation to deal with type names with irregular pluralization and the `@table` annotation to follow your organization's naming conventions.
:::

### Using unmanaged tables

Sometimes, you may want to expose a view or a foreign table in your database through Exograph APIs, but not have Exograph manage them for schema migration purposes. For this purpose, you may use the `managed=false` attribute of the `@table` annotation.

Consider the following view:

```sql
CREATE VIEW product_profits AS
SELECT
    p.id,
    p.name,
    p.sale_price,
    p.purchase_price,
    p.sale_price - p.purchase_price AS profit
FROM products p;
```

You can define a type for this view as follows:

```exo
@postgres
module TodoDatabase {
  @access(true)
  type Product {
    @pk id: Int = autoIncrement()
    name: String
    salePrice: Float
    purchasePrice: Float
  }

  // highlight-start
  @table(managed=false)
  @access(query=true, mutation=false)
  type ProductProfit {
    @pk id: Int
    name: String
    salePrice: Float
    purchasePrice: Float
    profit: Float
  }
  // highlight-end
}
```

Exograph will map the `ProductProfit` type to the `product_profits` view (it could also be a table, possibly a foreign table). However, Exograph will ignore the `ProductProfit` type during schema migration.

Exograph will apply access control and infer queries for the `ProductProfit` type as usual, including [aggregated queries](operations/queries.md#aggregate-query). If you have an unmanaged type representing a view, you will typically want to set `mutation=false`, thus removing the mutation APIs for it. However, this is more of a convention than a restriction imposed by Exograph. Therefore, you may put any other access control rules if you want to modify the underlying data through the unmanaged type.

If you allow mutation through a managed type for a view, you will want to make any derived fields read-only. For example, if you allow mutation through the `ProductProfit` type, you will want to make the `profit` field read-only.

```exo
  @table(managed=false)
  @access(query=true, mutation=true)
  type ProductProfit {
    @pk id: Int
    name: String
    salePrice: Float
    purchasePrice: Float
    // highlight-next-line
    @readonly profit: Float
  }
```

An unmanaged type may skip marking a field as `@pk` if it doesn't have a primary key. For such a type, Exograph offers only collection and aggregate queries. For example, the `ProductProfit` type will have the `productProfits` and `productProfitsAgg` query but not the `productProfit` query (which would take the primary key as an argument).

If you want to mark all types in a module as unmanaged, you can use the `@postgres` annotation with the `managed=false` attribute.

```exo
@postgres(managed=false)
module CommerceViews {
  ...
}
```

Here, all the types in the `CommerceViews` module will be unmanaged. However, you can override the managed state of a specific type using the `@table` annotation.

## Field-level customization

Exograph maps each field to a column in the database and infers a few other aspects of the column.

### Specifying column name

Exograph infers the column's name as the **snake_cased** version of the field name for a scalar-type column. For example, Exograph will map the `name` field to the `name` column, while it will map `ticketPrice` to `ticket_price`.

You can customize the column's name using the `@column` annotation. For example, if you want to map the `name` field to the `headline` column, you can use the `@column` annotation as follows:

```exo
type Concert {
  ...
  @column("headline") name: String
}
```

As discussed [earlier](defining-types.md#defining-a-relationship), Exograph will infer a relationship between these two types. In the following example, it infers that the foreign key column in the `Concert` table is `venue_id`. It does so by appending `_id` to the field's name (in this case, `venue`).

```exo
@postgres
module ConcertModule {
  type Concert {
    ...
    venue: Venue
  }

  type Venue {
    ...
    concerts: Set<Concert>?
  }
}
```

If you want to customize the name of the foreign key column, you can use the `@column` annotation. For example, to map the `venue` field to the `venue_pk` column instead of `venue_id`, you can use the following setup:

```exo
@postgres
module ConcertModule {
  type Concert {
    ...
    @column("venue_pk") venue: Venue
  }

  type Venue {
    ...
    concerts: Set<Concert>?
  }
}
```

If you use a [composite primary key](#composite-primary-key), you can specify a mapping for the foreign key columns as we will see [later](#customizing-foreign-key-column-names) using a variant of the `@column` annotation.

### Assigning primary key

The `@pk` annotation designates the primary key of a type. Typically, you will mark one of the fields as the primary key, but Exograph supports composite primary keys as well.

#### Auto-incrementing primary key

To use the primary key of integer type, specify the field to be of the `Int` type and set its default value of `autoIncrement()`. In the following example, the `id` field is the primary key, and it will be automatically assigned a value when you create a new concert. Behind the scenes, Exograph will use the `SERIAL` type in PostgreSQL by default, but you can customize it to use `SMALLSERIAL` or `BIGSERIAL` through the `@dbtype` annotation, as we will see [later](#customizing-field-type).

```exo
type Concert {
  @pk id: Int = autoIncrement()
  ...
}
```

With this arrangement, clients cannot specify the value of the `id` field when creating a concert. Exograph will automatically assign a value to the `id` field.

When you use the `autoIncrement()` function without an argument, Exograph will use the sequence named `{schema}.{table}_{column}_id_seq` by default (which is how Postgres handles `SERIAL` types). You can customize the sequence name using the `@autoIncrement` annotation. For example, if you want to use the sequence named `my_sequence` for the `id` field, you can use the following definition:

```exo
type Concert {
  @pk id: Int = autoIncrement("my_sequence")
  ...
}

type Venue {
  @pk id: Int = autoIncrement("my_sequence")
  ...
}
```

If you want to use sequence from a specific schema, you can use the `@autoIncrement` annotation. For example, if you want to use the sequence named `my_sequence` from the `my_schema` schema, you can use the following definition:

```exo
type Concert {
  @pk id: Int = autoIncrement("my_schema.my_sequence")
  ...
}

type Venue {
  @pk id: Int = autoIncrement("my_schema.my_sequence")
  ...
}
```

In either case, both the `Concert` and `Venue` types will use the the specified sequence to generate the primary key values. This kind of arrangement is useful when you want to address multiple entities just by their id (akin to how Github uses ids for issues, pull requests, etc.).

#### Auto-generated Uuid key

To use the primary key of Uuid type, specify the field's type to be `Uuid` type with a default value. Exograph supports two UUID generation methods:

- `generate_uuid()` - uses PostgreSQL's `gen_random_uuid()` function (built into PostgreSQL 13+)
- `uuidGenerateV4()` - uses PostgreSQL's `uuid_generate_v4()` function (requires `uuid-ossp` extension)
- `uuidGenerateV7()` - uses PostgreSQL's `uuidv7()` (requires Postgres version 18 or higher)

```exo
type Concert {
  @pk id: Uuid = generate_uuid()
  // OR
  @pk id: Uuid = uuidGenerateV4()
  // OR
  @pk id: Uuid = uuidGenerateV7()
  ...
}
```

In both cases, the `id` field will be automatically assigned a UUID value when you create a new concert. Exograph automatically adds the required PostgreSQL extension as needed.

#### User-assignable primary key

Auto-generated keys will be the most common form in your model. However, sometimes, you may want to assign a client-specifiable value. For example, you may want to let clients generate a UUID and use it when creating a new entity. You may also need user-assignable primary keys for integration with other systems where you want to sync the primary keys across systems.

To make the primary key user-assignable, skip the default value as follows.

```exo
type Venue {
  @pk id: Int
  ...
}
```

#### Composite primary key

You can also specify a composite primary key by marking multiple fields with the `@pk` annotation. For example, if you want to make the combination of the `firstName` and `lastName` fields the primary key of the `Person` type and the combination of the `street`, `city`, `state`, and `zip` fields the primary key of the `Address` type, you can use the following definition:

```exo
@postgres
module PeopleDatabase {
  @access(true)
  type Person {
    @pk firstName: String
    @pk lastName: String
    age: Int
    address: Address?
  }

  @access(true)
  type Address {
    @pk street: String
    @pk city: String
    @pk state: String
    @pk zip: Int
    people: Set<Person>?
  }
}
```

Other than marking multiple fields with `@pk`, all other aspects of the primary key are the same as for a single primary key.

### Customizing foreign key column names

When you introduce a field of a type with composite primary key (such as the `address` field in the `Person` type in the above example), Exograph automatically generates column names by combining the field name with each primary key field. For example, the generated column names in the `people` table would be `address_street`, `address_city`, `address_state`, and `address_zip`.

You can customize these column names using the `@column(mapping=...)` annotation with an object literal that maps each primary key field to a custom column name. For example, if you want to map the `address` field to the `addr_street`, `addr_city`, `addr_state`, and `addr_zip` columns, you can use the following definition:

```exo
@postgres
module PeopleDatabase {
  type Person {
    @pk firstName: String
    @pk lastName: String
    age: Int
    @column(mapping={street: "addr_street", city: "addr_city", state: "addr_state", zip: "addr_zip"})
    address: Address?
  }

  type Address {
    @pk street: String
    @pk city: String
    @pk state: String
    @pk zip: Int
    info: String?
  }
}
```

With this setup, the foreign key columns in the `people` table will be named `addr_street`, `addr_city`, `addr_state`, and `addr_zip` instead of the default names.

You can also provide a partial mapping. Any primary key fields not specified in the mapping will use the default naming convention:

```exo
@column(mapping={zip: "postal_code"}) address: Address?
```

This would generate columns `address_street`, `address_city`, `address_state`, and `postal_code`.

### Specifying a default value

The default value of a column is specified using an assignment in the field definition. For example, as we have seen in the [previous section](#assigning-primary-key), you can set the default value of an `Int` field to `autoIncrement()` to make it auto-incrementing and the default value of a `Uuid` field to `generate_uuid()` or `uuidGenerateV4()` or `uuidGenerateV7()` to make it auto-generated.

Similarly, you can set the default value of a scalar column to a constant value. For example, if you want to set the default value of the `price` field to `50`, you can use the following definition:

```exo
type Concert {
  ...
  price: Float = 50
}
```

Setting a default value affects mutations associated with the type. When creating a new concert, the `price` field will be optional, and the default value will be used if you don't specify a value.

You can set the default value to `now()` for all date and time field types. For example, if you want to set the default value of the `createdAt` field to the current time, you can use the following definition:

```exo
type Concert {
  ...
  createdAt: Instant = now()
}
```

When you create a new concert, the `createdAt` field will be set to optional, and Exograph will use =the current time if you don't specify a value.

### Marking read-only

In the previous example, the intention seems to set the `createdAt` field to the creation time. However, this requires clients to not specify a value for the `createdAt` field in `createConcert` or `createConcerts` API. Furthermore, it can be changed to another value when updating the concert (the field remains part of the `updateConcert` and `updateConcerts` API). Therefore, this arrangement doesn't reflect the intention.

Exograph provides the `@readonly` annotation to make a field read-only for such situations. For example, you can make the `createdAt` field read-only using the following definition:

```exo
type Concert {
  ...
  @readonly createdAt: Instant = now()
}
```

Now, the `createdAt` field will not be part of the `createConcert`, `createConcerts`, `updateConcert`, and `updateConcerts`APIs. As a result, the field's value will always be the creation time.

Note that a field marked with the `@readonly` annotation must also have a default value.

### Updating automatically

What if you want to introduce another field `updatedAt` that should be set automatically to the current time whenever the concert is updated? Exograph provides the `@update` annotation to achieve this. For example, you can make the `updatedAt` field update automatically using the following definition:

```exo
type Concert {
  ...
  @update updatedAt: Instant = now()
}
```

Every time a client updates a concert, Exograph will set the `updatedAt` field to its default value, `now()`&mdash;the current time.

Like the `@readonly` annotation, an `@update` field must have a default value. The default value usually is a function (like `now()`) that generates the value at runtime. It can be constant, but a plain `@readonly` is more appropriate. An `@update` field is also implicitly considered read-only. Consequently, it will not be part of the `createConcert`, `createConcerts`, `updateConcert`, and `updateConcerts` APIs.

You may use the `@update` annotation to track other aspects. For example, if you want to set the last modified version, you can use the `@update` annotation as follows:

```exo
type Concert {
  ...
  @update modificationVersion: Uuid = generate_uuid()
  // OR
  @update modificationVersion: Uuid = uuidGenerateV4()
  // OR
  @update modificationVersion: Uuid = uuidGenerateV7()
}
```

Whenever a client updates a concert, Exograph will set the `modificationVersion` field to a new UUID.

### Controlling Nullability

Exograph will make the column nullable if the field is optional. You can control nullability by adding the `?` suffix to the field type. For example, if you want to make the `name` field non-nullable, you can use the following definition:

```exo
type TicketPrice {
  ...
  price: Float
  details: String?
}
```

Here, the database schema will have the `price` field as non-nullable and the `details` field as nullable.

### Constraining Uniqueness

Often, you want to set a constraint on a field to make it unique. You may use the `@unique` annotation for this purpose. For example, if you want to make sure that the `name` field is unique, you can use the `@unique` annotation:

```exo
type Concert {
  ...
  @unique name: String
}
```

Here, Exograph will set a database uniqueness constraint on the `name` column in the generated schema.

:::info Uniqueness and queries
Exograph infers specialized queries to get an entity by unique fields. We will explore this in the [queries](operations/queries.md#unique-constraint-query) section.
:::

If you want to mark a specific combination of fields as unique, you can use the `@unique` annotation by specifying a name for the unique constraint. For example, if you want to ensure that the combination of `emailId` and `emailDomain` is unique, you can use the `@unique` annotation specifying a name for the unique constraint:

```exo
type Person {
  ...
  @unique("email") emailId: String
  @unique("email") emailDomain: String
}
```

You can pass an array of field names to the `@unique` annotation to specify a unique constraint. For example, if you want to make sure that the combination of `primaryEmailId` and `emailDomain` as well as the combination `secondaryEmailId` and `emailDomain` is unique, you can use the `@unique` annotation specifying names for the unique constraint:

```exo
type Person {
    @unique("primary_email") primaryEmailId: String
    @unique("secondary_email") secondaryEmailId: String?
    @unique("primary_email", "secondary_email") emailDomain: String
}
```

Since we use the name `primary_email` for the `primaryEmailId` and the `emailDomain` fields, that combination will be marked unique. We do the same for the `secondaryEmailId` and the `emailDomain` fields.

### Adding Index

It is a common practice to set up indexes on columns to speed up queries. While indexes speed up queries, they slow down inserts and updates. So, you should analyze the usage pattern of your application and create indexes accordingly.

By default, Exograph will not set up any explicit indexes. However, note that Postgres will set up indices for primary key columns and those with a uniqueness constraint (see `@unique` above). Exograph offers the `@index` annotation to allow you to set up appropriate indices. The `@index` annotation follows the same syntax as the `@unique` annotation. For example, if you want to create an index on the `age` column, you can use the `@index` annotation as follows:

```exo
type Person {
  ...
  @index age: Int
}
```

Here, Exograph will create an index named `person_age_idx` on the `age` column. If you want to control the name of the index, you can use the `@index` annotation as follows:

```exo
type Person {
  ...
  @index("person_age_index") age: Int
}
```

Suppose the application's usage pattern suggests that you must create an index on a combination of fields (typically, a frequent query that supplies a `where` clause with multiple fields). You can use the `@index` annotation by specifying a name for the index. For example, if you want to create an index on the combination of `firstName` and `lastName`, you can use the `@index` annotation specifying a name for the index:

```exo
type Person {
  ...
  @index("person_name") firstName: String
  @index("person_name") lastName: String
}
```

Here, Exograph will create an index with the name `person_name` on the columns for the `firstName` and `lastName` fields.

Like the `@unique` annotation, you can pass an array of field names to the `@index` annotation to specify an index. For example, suppose you need to create an index on the combination of `firstName` and `lastName` and those fields individually. You can use the `@index` annotation specifying names for the index:

```exo
type Person {
  ...
  @index("person_name", "person_first_name") firstName: String
  @index("person_name", "person_last_name") lastName: String
}
```

Here, Exograph will set up three indices: one on the `firstName` field, one on the `lastName` field, and one on the combination of the `firstName` and `lastName` fields.

### Customizing field type

Exograph infers the column type based on the field type. For example, if the field type is `String`, the column type will be inferred as `TEXT`. However, you may want more precise control over the database column type. Exograph offers a few annotations for this purpose.

#### Explicit column type

If you need to control the mapping of a database column type directly, you can use the `@dbtype` annotation. For example, if you want to set the column type of the `name` field to `VARCHAR(100)`, you can use the `@type` annotation:

```exo
@dbtype("VARCHAR(100)") name: String
```

The `@dbtype` annotation takes a string argument: the column type. You can use any valid PostgreSQL type compatible with the field type. For example, if you want to set the column type of the `price` field to `SMALLINT`, you can use the `@dbtype` annotation as follows:

```exo
@dbtype("SMALLINT") price: Int
```

:::warning
Ensure that the type you specify is compatible with the field type. For example, if you specify the type as `VARCHAR(100)` for a field of type `Int`, the generated schema will be invalid.
:::

The `@dbtype` annotation offers low-level control over the column type. Exograph also provides a few type-specific annotations. These annotations indirectly have the same effect as using the `@dbtype` annotation but offer a more convenient way to customize the column type.

:::warning
Specifying both the `@dbtype` and the type-specific annotations is an error.
:::

#### String field type

By default, the column type will allow the string to be of any length (to the extent the database allows it). However, you can use the `@maxLength` annotation to restrict it. For example, if you want to limit the length of the `name` field to 100 characters, you can use the `@maxLength` annotation:

```exo
@maxLength(100) description: String
```

Here, the column type will be set to `VARCHAR(100)` (instead of `TEXT`).

#### Integer field type

Exograph offers a few annotations for integer fields to customize the column type. They all have the effect of setting the column type to one of `SMALLINT`, `INTEGER`, or `BIGINT` in addition to setting constraints on the value of the field. The annotations are:

- `@bits16`, `@bits32`, and `bits64`: These annotations specify the number of bits in the integer. For example, if you want to set the column type of the `mask` field to contain only 16 bits, you can use the `@bits*` annotation as follows:

```exo
@bits16 mask: Int
```

You can specify only one of the `@bits*` annotations.

- `@range`: This annotation specifies the range of the integer. It takes two arguments: the minimum and the maximum value. For example, if you want to set the column type of the `age` field to 0 to 200, you can use the `@range` annotation:

```exo
@range(min = 0, max = 200) age: Int
```

You may use the `@range` annotation with one of the `@bits*` annotations. Exograph will infer the column type based on the range and integer size.

#### Float field type

Exograph offers the `@singlePrecision` and `@doublePrecision` annotations for float fields to customize the column type. They all have the effect of setting the column type to `REAL` or `DOUBLE PRECISION`.

#### Decimal field type

Floating point numbers are OK if you can tolerate some loss of precision. However, you can use the `Decimal` type to store the exact decimal numbers. For example, if you want to store the price of a concert ticket, you can use the `Decimal` type. For this type, Exograph offers the following annotations to customize the column type:

- `@precision`: This annotation specifies the total number of digits in the decimal number.

- `@scale`: This annotation specifies the number of digits after the decimal point. For example, if you want to set the column type of the `price` field to 2 digits after the decimal point, you can use the `@scale` annotation:

```exo
@precision(5) @scale(2) price: Decimal
```

The above will set the column type to `NUMERIC(5, 2)` and allow numeric values between 0.00 and 999.99. See the [PostgreSQL documentation](https://www.postgresql.org/docs/current/datatype-numeric.html#DATATYPE-NUMERIC-DECIMAL) for more details.

If you specify `@scale`, you must also specify `@precision`.

#### Date and Time field type

For [date and time fields](defining-types.md#defining-a-scalar-field) (`LocalDateTime`, `LocalDate`, and `Instance`), Exograph offers the `@precision`, which then maps it to Postgres's precision. See the [PostgreSQL documentation](https://www.postgresql.org/docs/current/datatype-datetime.html#DATATYPE-DATETIME-INPUT) for more details.
