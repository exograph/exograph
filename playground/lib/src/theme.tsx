import React, {
  useCallback,
  useEffect,
  useRef,
  useState,
  createContext,
  useContext,
} from "react";

export type Theme = "light" | "dark";

type ThemeContextType = {
  theme: Theme;
  setTheme: (theme: Theme) => void;
};

const ThemeContext = createContext<ThemeContextType | null>(null);

function getStoredTheme(): Theme | null {
  try {
    return localStorage.getItem("exograph-theme") as Theme;
  } catch {
    return null;
  }
}

function setStoredTheme(theme: Theme) {
  try {
    localStorage.setItem("exograph-theme", theme);
  } catch {
    // Silently fail if localStorage is not available
  }
}

function applyThemeToDocument(theme: Theme) {
  if (theme === "dark") {
    document.documentElement.classList.add("dark");
    document.documentElement.style.colorScheme = "dark";
  } else {
    document.documentElement.classList.remove("dark");
    document.documentElement.style.colorScheme = "light";
  }
}

export function ThemeProvider({ children }: { children: React.ReactNode }) {
  const mql = useRef(window.matchMedia("(prefers-color-scheme: dark)")).current;

  const getSystemTheme = useCallback((): Theme => {
    return mql.matches ? "dark" : "light";
  }, [mql]);

  const getCurrentTheme = useCallback((): Theme => {
    // Check if we have a stored preference first
    const stored = getStoredTheme();
    if (stored) {
      return stored;
    }
    // Otherwise check if dark class is applied to document
    if (document.documentElement.classList.contains("dark")) {
      return "dark";
    }
    // Finally fall back to system preference
    return getSystemTheme();
  }, [getSystemTheme]);

  const [theme, setThemeState] = useState<Theme>(() => getCurrentTheme());

  const setTheme = useCallback((newTheme: Theme) => {
    setThemeState(newTheme);
    setStoredTheme(newTheme);
    applyThemeToDocument(newTheme);
  }, []);

  useEffect(() => {
    const stored = getStoredTheme();
    if (!stored) {
      // No stored preference, follow system theme changes
      const handleSystemThemeChange = () => {
        const systemTheme = getSystemTheme();
        setThemeState(systemTheme);
        applyThemeToDocument(systemTheme);
      };
      mql.addEventListener("change", handleSystemThemeChange);
      return () => mql.removeEventListener("change", handleSystemThemeChange);
    }
  }, [mql, getSystemTheme]);

  // Apply theme to document on mount and changes
  useEffect(() => {
    applyThemeToDocument(theme);
  }, [theme]);

  return (
    <ThemeContext.Provider value={{ theme, setTheme }}>
      {children}
    </ThemeContext.Provider>
  );
}

export function useTheme(): Theme {
  const context = useContext(ThemeContext);
  if (!context) {
    throw new Error("useTheme must be used within a ThemeProvider");
  }
  return context.theme;
}

export function useSetTheme(): (theme: Theme) => void {
  const context = useContext(ThemeContext);
  if (!context) {
    throw new Error("useSetTheme must be used within a ThemeProvider");
  }
  return context.setTheme;
}
