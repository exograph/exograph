import type { Operation } from '../generated/exograph';

export async function time(operation: Operation) {
	return await operation.proceed();
}

