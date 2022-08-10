export async function callThrowClaytipError(claytip: Claytip): Promise<number> {
	const result = await claytip.executeQuery(
		`query {
			throwClaytipError
		}`
	);
	return result.data.throwClaytipError;
}

export async function callThrowClaytipErrorPriv(claytip: ClaytipPriv): Promise<number> {
	const result = await claytip.executeQueryPriv(
		`query {
			throwClaytipError
		}`
	);
	return result.data.throwClaytipError;
}

export async function throwClaytipError(): Promise<number> {
	throw new ClaytipError('user message');
}

