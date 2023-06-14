import type { Operation } from '../generated/exograph.d.ts';

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
