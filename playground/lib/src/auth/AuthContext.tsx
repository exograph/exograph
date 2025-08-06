import React, { useCallback, useState, createContext } from "react";
import { ClerkAuthPlugin } from "./clerk";
import { SecretAuthPlugin } from "./secret";
import { Auth0AuthPlugin } from "./auth0";
import { AuthPlugin } from "./AuthPlugin";
import { JwtSecret } from "./secret/SecretConfig";

type GetToken = () => Promise<string | null>;

export function updateLocalStorage(key: string, value?: string): void {
  if (value) {
    localStorage.setItem(key, value);
  } else {
    localStorage.removeItem(key);
  }
}

type AuthContextType<C> = {
  plugin: AuthPlugin<C>;

  getTokenFn?: GetToken;
  setTokenFn?: (getToken: GetToken | undefined) => void;

  getSignOutFn?: () => Promise<void>;
  setSignOutFn?: (signOut: () => Promise<void>) => void;

  isSignedIn?: boolean;
  setIsSignedIn?: (isSignedIn?: boolean) => void;

  userInfo?: string;
  setUserInfo?: (userInfo?: string) => void;
};

export const AuthContext = createContext<AuthContextType<any>>(
  null as any as AuthContextType<any>
);

export function AuthContextProvider({
  oidcUrl,
  jwtSecret,
  children,
}: {
  oidcUrl?: string;
  jwtSecret?: JwtSecret;
  children: React.ReactNode;
}) {
  const [isSignedIn, setIsSignedIn] = useState<boolean | undefined>(false);
  const [getTokenFn, setTokenFn] = useState<GetToken | undefined>();
  const [getSignOutFn, setSignOutFn] = useState<() => Promise<void>>();

  const [userInfo, setUserInfo] = useState<string | undefined>();

  const authenticatorType =
    oidcUrl && oidcUrl.endsWith("auth0.com")
      ? "auth0"
      : oidcUrl && oidcUrl.endsWith("clerk.accounts.dev")
        ? "clerk"
        : "secret";

  const plugin: AuthPlugin<any> =
    authenticatorType === "clerk"
      ? new ClerkAuthPlugin()
      : authenticatorType === "auth0"
        ? new Auth0AuthPlugin()
        : new SecretAuthPlugin(jwtSecret);

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
        plugin,
        getTokenFn,
        setTokenFn: setTokenFnCb,
        getSignOutFn,
        setSignOutFn: setSignOutCb,
        userInfo,
        setUserInfo,
      }}
    >
      {children}
    </AuthContext.Provider>
  );
}
