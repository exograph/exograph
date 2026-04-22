import { useTheme } from "./theme";

import { default as DarkLogo } from "../../public/logo-dark.svg";
import { default as LightLogo } from "../../public/logo-light.svg";

export function Logo() {
  let theme = useTheme();

  const ThemedLogo = theme === "dark" ? DarkLogo : LightLogo;

  return (
    <div className="flex items-center h-10">
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
