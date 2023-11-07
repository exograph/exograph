import os from "node:os";

export function test_builtin(): string {
	return os.arch();
}
