import { useTheme } from "./theme";

import { default as DarkLogo } from "../public/logo-dark.svg";
import { default as LightLogo } from "../public/logo-light.svg";

function ExographLogo() {
  const theme = useTheme();

  // Currently, switching mode in GraphiQL doesn't update the logo, but this will get fixed
  // when https://github.com/graphql/graphiql/pull/2971 is merged.
  // Vite treats imports as strings, but Webpack (in Docusaurus configuration) treats them as React components.
  // So, we use `vite-plugin-svgr` in Vite vite to convert SVGs to React components, but we need to cast them to `any` to avoid TypeScript errors.
  const Logo = (theme === "dark" ? DarkLogo : LightLogo) as any;

  return (
    <a
      href="https://exograph.dev"
      target="_blank"
      rel="noreferrer"
      style={{ lineHeight: 0 }} // Otherwise, the logo is not vertically centered
    >
      <Logo className="logo" alt="Exograph" />
    </a>
  );
}

export function Logo() {
  return (
    <div style={{ display: "flex", alignItems: "center", gap: "1rem" }}>
      <ExographLogo />
    </div>
  );
}
