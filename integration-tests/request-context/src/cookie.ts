import "./exograph.d.ts"

export function getCookie(exograph: Exograph): boolean {
	exograph.setCookie({
		name: "session_id",
		value: "abcde"
	});

	return true
}
