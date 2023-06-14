import type { Operation } from 'https://deno.land/x/exograph@v0.0.5/index.ts';

export function todoReturnFreshValue(_operation: Operation) {
	// Intercept the operation and return a fresh value i.e. not the one returned by `operation.proceed()`
	return {
		id: 1,
		title: 'Test'
	}
}

export function infoReturnFreshValue(_operation: Operation) {
	// Intercept the operation and return a fresh value i.e. not the one returned by `operation.proceed()`
	return {
		id: 1,
		title: 'Test'
	}
}
