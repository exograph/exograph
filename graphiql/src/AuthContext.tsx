import React, { useCallback, useState } from "react";
import { ClerkAuthPlugin } from "./auth/clerk";
import { SecretAuthPlugin } from "./auth/secret";
import { Auth0AuthPlugin } from "./auth/auth0";
import { AuthPlugin } from "./AuthPlugin";

type GetToken = () => Promise<string | null>;

const exoOidcUrl: string = (window as any).exoOidcUrl;

const authenticatorType =
  exoOidcUrl && exoOidcUrl.endsWith("auth0.com")
    ? "auth0"
    : exoOidcUrl && exoOidcUrl.endsWith("clerk.accounts.dev")
    ? "clerk"
    : "secret";

const plugin: AuthPlugin =
  authenticatorType === "clerk"
    ? new ClerkAuthPlugin()
    : authenticatorType === "auth0"
    ? new Auth0AuthPlugin()
    : new SecretAuthPlugin();

export function updateLocalStorage(key: string, value?: string): void {
  if (value) {
    localStorage.setItem(key, value);
  } else {
    localStorage.removeItem(key);
  }
}

type AuthContextType = {
  plugin: AuthPlugin;

  getTokenFn?: GetToken;
  setTokenFn?: (getToken: GetToken | undefined) => void;

  getSignOutFn?: () => Promise<void>;
  setSignOutFn?: (signOut: () => Promise<void>) => void;

  isSignedIn?: boolean;
  setIsSignedIn?: (isSignedIn?: boolean) => void;

  userInfo?: string;
  setUserInfo?: (userInfo?: string) => void;
};

const defaultContext: AuthContextType = {
  plugin,
};

export const AuthContext = React.createContext<AuthContextType>(defaultContext);

export function AuthContextProvider(props: { children: React.ReactNode }) {
  const [isSignedIn, setIsSignedIn] = useState<boolean | undefined>(false);
  const [getTokenFn, setTokenFn] = useState<GetToken | undefined>();
  const [getSignOutFn, setSignOutFn] = useState<() => Promise<void>>();

  const [userInfo, setUserInfo] = useState<string | undefined>();

  const setTokenFnCb = useCallback(
    (f: GetToken | undefined) => {
      setTokenFn(() => f);
    },
    [setTokenFn]
  );

  const setSignOutCb = useCallback(
    (f: () => Promise<void>) => {
      setSignOutFn(() => f);
    },
    [setSignOutFn]
  );

  return (
    <AuthContext.Provider
      value={{
        isSignedIn,
        setIsSignedIn,
        plugin: defaultContext.plugin,
        getTokenFn,
        setTokenFn: setTokenFnCb,
        getSignOutFn,
        setSignOutFn: setSignOutCb,
        userInfo,
        setUserInfo,
      }}
    >
      {props.children}
    </AuthContext.Provider>
  );
}
