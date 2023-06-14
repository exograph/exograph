import { Operation } from '../generated/exograph.d.ts';

export async function time(operation: Operation) {
	return await operation.proceed();
}

