interface Todo {
  id: number
  userId: number
  title: string
  completed: boolean
}

export async function todo(id: number): Promise<Todo> {
  const r = await fetch(`https://jsonplaceholder.typicode.com/todos/${id}`);
  return r.json();
}

