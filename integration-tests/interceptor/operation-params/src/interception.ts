import type { Operation } from '../generated/exograph.d.ts';

interface OperationParams {
	name: string
	query: string
}

export function serve(_intArg: number, _stringArg: string): OperationParams | null {
	return null;
}

export function captureParams(operation: Operation) {
	return {
		name: operation.name(),
		query: JSON.stringify(operation.query())
	}
}

