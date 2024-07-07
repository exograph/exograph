import type { Todo } from "../generated/TodoModule.d.ts";

export async function todo(id: number): Promise<Todo> {
  const r = await fetch(`https://jsonplaceholder.typicode.com/todos/${id}`);
  return r.json();
}
