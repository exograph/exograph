import type { Exograph } from '../generated/exograph.d.ts';

export async function getCookie(exograph: Exograph): Promise<boolean> {
	exograph.setCookie({
		name: "session_id",
		value: "abcde"
	});

	return true
}
