---
sidebar_position: 6
---

# Imports

When an Exograph model gets large, it is a good idea to organize it much like in any software project. Exograph offers the import mechanism to help you organize your model.

An exograph file may import another exograph file using the `import` keyword.

```exo
import "monitoring.exo"
import "authentication.exo"
```

Exograph interprets an imported file as if its source was included directly (except it takes care of dealing with recursive imports--it is okay to have "a.exo" import "b.exo" and "b.exo" import "a.exo").
