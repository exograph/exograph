import { useTheme } from "./theme";

import { default as DarkLogo } from "../../public/logo-dark.svg";
import { default as LightLogo } from "../../public/logo-light.svg";

export function Logo() {
  let theme = useTheme();

  // Vite treats imports as strings, but Webpack (in Docusaurus configuration) treats them as React components.
  // So, we use `vite-plugin-svgr` in Vite vite to convert SVGs to React components, but we need to cast them to `any` to avoid TypeScript errors.
  const ThemedLogo = (theme === "dark" ? DarkLogo : LightLogo) as any;

  return (
    <div className="flex items-center h-8">
      <a
        href="https://exograph.dev"
        target="_blank"
        rel="noreferrer"
        className="h-full"
      >
        <ThemedLogo className="h-full w-auto" alt="Exograph" />
      </a>
    </div>
  );
}
