import type { Exograph } from '../generated/exograph.d.ts';
import type * as TodoDatabase from '../generated/TodoDatabase.d.ts';

// The return type TodoDatabase.Todo is imported from the generated .d.ts file
export async function completedTodos(_exograph: Exograph): Promise<TodoDatabase.Todo[]> {
  // Return a static list to demonstrate the cross-module type works
  return [
    { id: 1, title: "Test todo", completed: true }
  ];
}
