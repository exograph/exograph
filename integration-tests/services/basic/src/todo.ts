import type { Todo, Todos } from "../generated/TodoModule.d.ts";

export async function todo(id: number): Promise<Todo> {
  const r = await fetch(`https://jsonplaceholder.typicode.com/todos/${id}`);
  return r.json();
}

export async function todos(): Promise<Todos> {
  const r = await fetch(`https://jsonplaceholder.typicode.com/todos`);
  const todos = await r.json();
  return {
    totalCount: todos.length,
    items: todos,
  };
}
