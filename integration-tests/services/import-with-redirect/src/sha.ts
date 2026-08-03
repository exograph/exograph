// Importing directly from raw.githubusercontent.com would not test redirect handling.
import { encode as hexify } from "https://github.com/denoland/std/raw/0.192.0/encoding/hex.ts";

const decode = (d: Uint8Array) => new TextDecoder().decode(d);

export async function computeSha512(value: string): Promise<string> {
	const sha = await crypto.subtle.digest("SHA-512", new TextEncoder().encode(value));
	return decode(hexify(new Uint8Array(sha)));
}

