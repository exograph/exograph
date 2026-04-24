export function greet(name: string | undefined | null): string {
	if (name === undefined || name === null) {
		return "Hello, stranger!";
	}
	return `Hello, ${name}!`;
}

export function greetFormal(
	title: string | undefined | null,
	name: string,
	suffix: string | undefined | null,
): string {
	const parts: string[] = [];
	if (title !== undefined && title !== null) parts.push(title);
	parts.push(name);
	if (suffix !== undefined && suffix !== null) parts.push(suffix);
	return `Hello, ${parts.join(" ")}!`;
}
