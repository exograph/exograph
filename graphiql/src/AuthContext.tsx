import React, { useCallback, useState } from "react";

type GetToken = () => Promise<string | null>;

export type AuthenticatorInfo =
  | SecretAuthenticatorInfo
  | ClerkAuthenticatorInfo;

export type ClerkAuthenticatorInfo = {
  type: "clerk";
  publishableKey: string | undefined;
  templateId: string | undefined;
};

export type SecretAuthenticatorInfo = {
  type: "secret";
};

interface AuthContextType {
  isSignedIn?: boolean;
  setIsSignedIn?: (isSignedIn?: boolean) => void;
  getTokenFn?: GetToken;
  setTokenFn?: (getToken: GetToken | undefined) => void;
  authenticatorInfo: AuthenticatorInfo;
  setAuthenticatorInfo?: (info: AuthenticatorInfo) => void;
}

const exoJwksBaseUrl: string = (window as any).exoJwksBaseUrl;

const authenticatorType =
  exoJwksBaseUrl && exoJwksBaseUrl.endsWith("clerk.accounts.dev")
    ? "clerk"
    : "secret";

const publishableKeyKey = "exograph:clerkPublishableKey";
const templateIdKey = "exograph:clerkTemplateId";

const initAuthenticatorInfo: AuthenticatorInfo =
  authenticatorType === "clerk"
    ? {
        type: authenticatorType,
        publishableKey: localStorage.getItem(publishableKeyKey) || undefined,
        templateId: localStorage.getItem(templateIdKey) || undefined,
      }
    : {
        type: authenticatorType,
      };

export const AuthContext = React.createContext<AuthContextType>({
  authenticatorInfo: initAuthenticatorInfo,
});

export function AuthContextProvider(props: { children: React.ReactNode }) {
  const [isSignedIn, setIsSignedIn] = useState<boolean | undefined>(false);
  const [getTokenFn, setTokenFn] = useState<GetToken | undefined>();
  const [authenticatorInfo, setAuthenticatorInfo] = useState<AuthenticatorInfo>(
    initAuthenticatorInfo
  );

  const setTokenFnCb = useCallback(
    (f: GetToken | undefined) => {
      setTokenFn(() => f);
    },
    [setTokenFn]
  );

  const setAuthenticatorInfoCb = useCallback(
    (info: AuthenticatorInfo) => {
      setAuthenticatorInfo(info);
      if (info.type === "clerk") {
        if (info.publishableKey) {
          localStorage.setItem(publishableKeyKey, info.publishableKey);
        } else {
          localStorage.removeItem(publishableKeyKey);
        }
        if (info.templateId) {
          localStorage.setItem(templateIdKey, info.templateId);
        } else {
          localStorage.removeItem(templateIdKey);
        }
      }
    },
    [setAuthenticatorInfo]
  );

  return (
    <AuthContext.Provider
      value={{
        isSignedIn,
        setIsSignedIn,
        getTokenFn,
        setTokenFn: setTokenFnCb,
        authenticatorInfo,
        setAuthenticatorInfo: setAuthenticatorInfoCb,
      }}
    >
      {props.children}
    </AuthContext.Provider>
  );
}
