export async function time(operation: Operation) {
	console.log('time');
	return await operation.proceed();
}

