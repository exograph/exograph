import * as tiny_math from "https://deno.land/x/tiny_math@0.1.3/mod.ts";

export function gcd(a: number, b: number): number {
	return tiny_math.gcd(a, b);
}

export function lcm(a: number, b: number): number {
	return tiny_math.lcm(a, b);
}

