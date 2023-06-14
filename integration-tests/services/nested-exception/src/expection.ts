import type { Exograph, ExographPriv } from 'https://deno.land/x/exograph@v0.0.5/index.ts';
// import { ExographError } from "https://deno.land/x/exograph@v0.0.5/index.ts";

export async function callThrowExographError(exograph: Exograph): Promise<number> {
	const result = await exograph.executeQuery(
		`query {
			throwExographError
		}`
	);
	return result.data.throwExographError;
}

export async function callThrowExographErrorPriv(exograph: ExographPriv): Promise<number> {
	const result = await exograph.executeQueryPriv(
		`query {
			throwExographError
		}`
	);
	return result.data.throwExographError;
}

export async function throwExographError(): Promise<number> {
	throw new ExographError('user message');
}

