export async function todoReturnFreshValue(operation: Operation) {
	// Intercept the operation and return a fresh value i.e. not the one returned by `operation.proceed()`
	return {
		id: 1,
		title: 'Test'
	}
}

export async function infoReturnFreshValue(operation: Operation) {
	// Intercept the operation and return a fresh value i.e. not the one returned by `operation.proceed()`
	return {
		id: 1,
		title: 'Test'
	}
}
