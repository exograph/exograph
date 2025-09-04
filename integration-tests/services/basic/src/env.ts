export async function getEnv(key: string): Promise<string | undefined> {
	return Deno.env.get(key);
}

