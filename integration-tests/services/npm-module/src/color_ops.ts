import Color from "npm:color";

export function lighten_color(color: string): string {
	const lightened = Color(color).lighten(0.5);
	return lightened.hex();
}
