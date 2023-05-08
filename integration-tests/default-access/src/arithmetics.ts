interface DivisionResultNoAccess {
	quotient: number
	remainder: number
}

interface DivisionResultFullAccess {
	quotient: number
	remainder: number
}

export async function divideFullAccess(a: number, b: number): Promise<DivisionResultNoAccess> {
	return {
		quotient: Math.floor(a / b),
		remainder: a % b
	}
}

export async function divideFullAccessMutation(a: number, b: number): Promise<DivisionResultNoAccess> {
	return {
		quotient: Math.floor(a / b),
		remainder: a % b
	}
}

export async function divideNoAccess(a: number, b: number): Promise<DivisionResultFullAccess> {
	return {
		quotient: Math.floor(a / b),
		remainder: a % b
	}
}

export async function divideNoAccessMutation(a: number, b: number): Promise<DivisionResultFullAccess> {
	return {
		quotient: Math.floor(a / b),
		remainder: a % b
	}
}

export async function divide(a: number, b: number): Promise<DivisionResultFullAccess> {
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
