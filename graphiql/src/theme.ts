
import { useTheme as useGraphiqlTheme } from "@graphiql/react";
import { useCallback, useEffect, useRef, useState } from "react";

type Theme = 'light' | 'dark';

function useBrowserTheme(): Theme {
  const mql = useRef(window.matchMedia("(prefers-color-scheme: dark)")).current;

  const currentTheme = useCallback(() => {
    return mql.matches ? "dark" : "light";
  }, [mql]);

  const [theme, setTheme] = useState<Theme>(currentTheme());

  useEffect(() => {
    const setCurrentTheme = () => {
      setTheme(currentTheme());
    };
    mql.addEventListener("change", setCurrentTheme);
    return () => mql.removeEventListener("change", setCurrentTheme);
  }, [currentTheme, mql]);

  return theme;
};

export function useTheme(): Theme {
  const graphiqlTheme = useGraphiqlTheme().theme;
  const browserTheme = useBrowserTheme();

  // Fallback to the browser's theme if GraphiQL's theme is set to "System" (which will make `graphiqlTheme` as null)
  if (graphiqlTheme === null) {
    return browserTheme;
  }
  return graphiqlTheme;
}
