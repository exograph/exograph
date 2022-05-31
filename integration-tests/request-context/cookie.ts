import "./claytip.d.ts"

export function getCookie(claytip: Claytip): boolean {
	claytip.setCookie({
		name: "session_id",
		value: "abcde"
	});

	return true
}
