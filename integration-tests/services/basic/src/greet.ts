export function greet(name: string | undefined | null): string {
	if (name === undefined || name === null) {
		return "Hello, stranger!";
	}
	return `Hello, ${name}!`;
}
