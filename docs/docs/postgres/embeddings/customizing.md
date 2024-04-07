---
sidebar_position: 40
---

# Customizing

Until now, we have used the default settings for embeddings. However, Exograph provides several ways to customize embeddings:

- **Size**: The size of the vector. By default, Exograph uses a size of 1536, but you can specify a different size using the `@size` annotation. Exograph's schema creation and migration will factor in the vector size.

- **Indexing**: Creating indexes speeds up the search and ordering. Exograph's `@index` annotation on the `contentVector` field indicates the need to create an index. During schema creation (and migration), Exograph sets up a [Hierarchical Navigable Small World (HNSW)](https://en.wikipedia.org/wiki/Hierarchical_Navigable_Small_World_graphs) index.

- **Distance function**: The core motivation for using vectors is to find vectors similar to a target. There are multiple ways to compute similarity, and based on the field's characteristics, one may be more suitable than others. Since it is a field's characteristic, you can annotate `Vector` fields using the `@distanceFunction` annotation to specify the distance function. By default, Exograph uses the "cosine" distance function, but you can also use the "l2" distance function (L2 or Euclidean distance) or "ip" (inner product). Exograph will automatically use this function when setting up filters and ordering. It will also automatically factor in the distance function while setting up the index.

For example, to customize the `contentVector` field, you can use the following schema:

```exo
@postgres
module DocumentModule {
    @access(true)
    type Document {
        @pk id: Int = autoIncrement()
        title: String
        content: String

        // highlight-start
        @size(1536)
        @index
        @distanceFunction("l2")
        contentVector: Vector?
        // highlight-end
    }
}
```

With these annotations, Exograph will set the vector size to 1536, use the L2 distance function, and create an index on the `contentVector` field.
