import React, { useState, useEffect, useRef, useCallback } from "react";

import { useTheme } from "@graphiql/react";

export const useBrowserTheme = () => {
  const mql = useRef(window.matchMedia("(prefers-color-scheme: dark)")).current;

  const currentTheme = useCallback(() => {
    return mql.matches ? "dark" : "light";
  }, [mql]);

  const [theme, setTheme] = useState(currentTheme());

  useEffect(() => {
    const setCurrentTheme = () => {
      setTheme(currentTheme());
    };
    mql.addEventListener("change", setCurrentTheme);
    return () => mql.removeEventListener("change", setCurrentTheme);
  }, [currentTheme, mql]);

  return theme;
};

function ExographLogo() {
  const graphiqlTheme = useTheme().theme;
  // Fallback to the browser's theme if GraphiQL's theme is set to "System" (which will name `graphiqlTheme` as null)
  // If the user switches theme in the browser, the logo will be updated accordingly
  const browserTheme = useBrowserTheme();

  const effectiveTheme = graphiqlTheme || browserTheme;

  // Currently, switching mode in GraphiQL doesn't update the logo, but this will get fixed
  // when https://github.com/graphql/graphiql/pull/2971 is merged.
  const logo = effectiveTheme === "dark" ? "logo-dark.svg" : "logo-light.svg";

  return (
    <a
      href="https://exograph.dev"
      target="_blank"
      rel="noreferrer"
      style={{ lineHeight: 0 }} // Otherwise, the logo is not vertically centered
    >
      <img src={logo} className="logo" alt="Exograph" />
    </a>
  );
}

export function Logo() {
  return (
    <div style={{ display: "flex", alignItems: "center", gap: "1rem" }}>
      <ExographLogo />
      {/* <UserProfile /> */}
    </div>
  );
}
