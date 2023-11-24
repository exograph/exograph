import Color from "npm:color"; // CommonJS
import tinycolor from "npm:tinycolor2"; // ESM
import { Resend } from "npm:resend"; // Module with namespace (indirectly "@react-email/render")

// Needed to trigger dynamic import
const resend = new Resend('fake-api-key');

export function lighten_color(color: string): string {
	const lightened = Color(color).lighten(0.5);
	return lightened.hex();
}

export function color_by_name(name: string): string {
	return tinycolor(name).toHexString();
}
