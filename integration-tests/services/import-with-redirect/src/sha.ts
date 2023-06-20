import { sha512 } from 'https://denopkg.com/chiefbiiko/sha512/mod.ts';
// import { sha512 } from 'https://raw.githubusercontent.com/chiefbiiko/sha512/master/mod.ts';

import { encode as hexify } from "https://deno.land/std@0.192.0/encoding/hex.ts";

const decode = (d: Uint8Array) => new TextDecoder().decode(d);

export function computeSha512(value: string): string {
	const sha = sha512(value);
	if (typeof sha === 'string') {
		return sha;
	} else {
		return decode(hexify(sha));
	}
}

