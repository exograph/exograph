import { Operation } from 'https://deno.land/x/exograph@v0.0.5/index.ts';

export async function time(operation: Operation) {
	return await operation.proceed();
}

