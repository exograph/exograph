interface DivisionResultNoAccess {
	quotient: number
	remainder: number
}

interface DivisionResultFullAccess {
	quotient: number
	remainder: number
}

export function divideFullAccess(a: number, b: number): DivisionResultNoAccess {
	return {
		quotient: Math.floor(a / b),
		remainder: a % b
	}
}

export function divideFullAccessMutation(a: number, b: number): DivisionResultNoAccess {
	return {
		quotient: Math.floor(a / b),
		remainder: a % b
	}
}

export function divideNoAccess(a: number, b: number): DivisionResultFullAccess {
	return {
		quotient: Math.floor(a / b),
		remainder: a % b
	}
}

export function divideNoAccessMutation(a: number, b: number): DivisionResultFullAccess {
	return {
		quotient: Math.floor(a / b),
		remainder: a % b
	}
}

export function divide(a: number, b: number): DivisionResultFullAccess {
	return {
		quotient: Math.floor(a / b),
		remainder: a % b
	}
}


export async function divideMutation(a: number, b: number): Promise<DivisionResultFullAccess> {
	return {
		quotient: Math.floor(a / b),
		remainder: a % b
	}
}
