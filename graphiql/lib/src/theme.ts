import { useCallback, useEffect, useRef, useState } from "react";

export type Theme = 'light' | 'dark';

export function useTheme(): Theme {
  const graphiqlTheme = useGraphiqlTheme();
  const browserTheme = useBrowserTheme();

  // Fallback to the browser's theme if GraphiQL's theme is set to "System" (which will make `graphiqlTheme` as null)
  if (!graphiqlTheme) {
    return browserTheme;
  }
  return graphiqlTheme;
}

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

// Detect changes to GraphiQL's theme based on the class of the body element (the useTheme from GraphiQL doesn't work for resetting logo)
function useGraphiqlTheme(): Theme | null {
  const [theme, setTheme] = useState<Theme | null>(null);

  useEffect(() => {
    const observer = new MutationObserver(() => {
      const bodyClass = document.body.classList;
      if (bodyClass.contains("graphiql-light")) {
        setTheme("light");
      } else if (bodyClass.contains("graphiql-dark")) {
        setTheme("dark");
      } else {
        setTheme(null);
      }
    });
    observer.observe(document.body, { attributeFilter: ["class"] })

    return () => { observer.disconnect() }

  }, []);

  return theme;

}
