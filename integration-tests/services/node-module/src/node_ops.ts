import process from "node:process";

export function test_builtin(): string {
	return process.cwd().length > 0 ? "OK" : "FAIL";
}
