---
sidebar_position: 3
---

# Customizing Schema

Exograph infers the mapping of a type to a table, which is often good enough. But if you need to customize the mapping, Exograph provides a few annotations. This section will examine the default Exograph mapping and how to customize it.

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

## Field-level customization

Exograph maps each field to a column in the database and infers a few other aspects of the column.

### Column name

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

If you want to customize the name of the foreign key column, you can use the `@column` annotation. For example, to map the `venue` field to the `venue_pk_` column instead of `venue_id`, you can use the following setup:

```exo
@postgres
module ConcertModule {
  type Concert {
    ...
    @column("venue_pk") venue: Venue
  }

  type Venue {
    ...
    @column("venue_pk") concerts: Set<Concert>?
  }
}
```

If you change the name of the foreign key column in the `Venue` type, you must also change the name of the foreign key column in the `Concert` type. This way, the column names guide Exograph to infer the relationship between the two types.

### Primary key

The `@pk` annotation designates the primary key of a type. The current implementation of Exograph only supports a single primary key (we will lift this restriction in the future):

#### Auto-incrementing primary key

To use the primary key of integer type, specify the field to be of the `Int` type and set its default value of `autoIncrement()`. In the following example, the `id` field is the primary key, and it will be automatically assigned a value when you create a new concert. Behind the scenes, Exograph will use the `SERIAL` type in PostgreSQL by default, but you can customize it to use `SMALLSERIAL` or `BIGSERIAL` through the `@dbtype` annotation, as we will see [later](#customizing-field-type).

```exo
type Concert {
  @pk id: Int = autoIncrement()
  ...
}
```

With this arrangement, clients cannot specify the value of the `id` field when creating a concert. Exograph will automatically assign a value to the `id` field.

#### Auto-generated Uuid key

To use the primary key of Uuid type, specify the field's type to be `Uuid` type with the default value of `generate_uuid()`. In the following example, the `id` field is the primary key, and it will be automatically assigned a value when you create a new concert. Behind the scenes, Exograph will use the `UUID` type in PostgreSQL.

```exo
type Concert {
  @pk id: Uuid = generate_uuid()
  ...
}
```

#### User-assignable primary key

Auto-generated keys will be the most common form in your model. However, sometimes, you may want to assign a client-specifiable value. For example, you may want to let clients generate a UUID and use it when creating a new entity. You may also need user-assignable primary keys for integration with other systems where you want to sync the primary keys across systems.

To make the primary key user-assignable, skip the default value as follows.

```exo
type Venue {
  @pk id: Int
  ...
}
```

### Default value

The default value of a column is specified using an assignment in the field definition. For example, as we have seen in the [previous](#primary-key)[ section](#primary-key), you can set the default value of an `Int` field to `autoIncrement()` to make it auto-incrementing and the default value of a `Uuid` field to `generate_uuid()` to make it auto-generated.

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
  createdAt: LocalDateTime = now()
}
```

When you create a new concert, the `createdAt` field will be set to optional, and the current time will be used if you don't specify a value.

#### Controlling Nullability

Exograph will make the column nullable if the field is optional. You can control nullability by adding the `?` suffix to the field type. For example, if you want to make the `name` field non-nullable, you can use the following definition:

```exo
type TicketPrice {
  ...
  price: Float
  details: String?
}
```

Here, the database schema will have the `price` field as non-nullable and the `details` field as nullable.

### Uniqueness

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

### Index

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
