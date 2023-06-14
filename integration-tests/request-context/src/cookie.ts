import type { Exograph } from 'https://deno.land/x/exograph@v0.0.5/index.ts';

export async function getCookie(exograph: Exograph): Promise<boolean> {
	exograph.setCookie({
		name: "session_id",
		value: "abcde"
	});

	return true
}
