interface OperationParams {
	name: string
	query: string
}

export async function serve(intArg: number, stringArg: string): Promise<OperationParams | null> {
	return null;
}

export async function captureParams(operation: Operation) {
	return {
		name: operation.name(),
		query: JSON.stringify(operation.query())
	}
}

