import React, { useCallback, useContext, useEffect, useState } from "react";

import { ClerkProvider, useAuth, useUser } from "@clerk/clerk-react";
import { dark } from "@clerk/themes";

import { AuthContext } from "../../AuthContext";
import { useTheme } from "../../theme";
import { ClerkConfig } from "./ClerkConfig";

type AuthConfig = {
  config: ClerkConfig;
  setConfig: (config: ClerkConfig) => void;
};

export const AuthConfigContext = React.createContext({} as AuthConfig);

export function AuthConfigProvider(props: { children: React.ReactNode }) {
  const [config, setConfig] = useState(ClerkConfig.loadConfig());
  const theme = useTheme();

  useEffect(() => {
    config.saveConfig();
  }, [config]);

  return (
    <AuthConfigContext.Provider
      value={{
        config,
        setConfig,
      }}
    >
      {config.canSignIn() && config.publishableKey ? (
        <ClerkProvider
          publishableKey={config.publishableKey}
          appearance={{ baseTheme: theme === "dark" ? dark : undefined }}
        >
          <ContextInitializer>{props.children}</ContextInitializer>
        </ClerkProvider>
      ) : (
        <>{props.children}</>
      )}
    </AuthConfigContext.Provider>
  );
}

function ContextInitializer(props: { children: React.ReactNode }) {
  const { isSignedIn, user } = useUser();
  const { getToken, signOut } = useAuth();
  const { setTokenFn, setIsSignedIn, setUserInfo, setSignOutFn } =
    useContext(AuthContext);
  const { config } = useContext(AuthConfigContext);

  const signOutFn = useCallback(async () => {
    signOut();
  }, [signOut]);

  const getUserInfo = useCallback(() => {
    let userInfo = "";
    if (user) {
      if (user.fullName && user.emailAddresses.length > 0) {
        userInfo = `${user.fullName} (${user.emailAddresses[0].emailAddress})`;
      } else if (user.fullName) {
        userInfo = user.fullName;
      } else if (user.emailAddresses.length > 0) {
        userInfo = user.emailAddresses[0].emailAddress;
      } else {
        userInfo = "Unknown";
      }
    }
    return userInfo;
  }, [user]);

  useEffect(() => {
    let templateId = config.templateId;
    const getTokenOptions = templateId ? { template: templateId } : undefined;
    setTokenFn &&
      setTokenFn(isSignedIn ? () => getToken(getTokenOptions) : undefined);
    setIsSignedIn && setIsSignedIn(isSignedIn);
    setUserInfo && setUserInfo(getUserInfo());
    setSignOutFn && setSignOutFn(() => signOutFn());
  }, [
    config,
    getToken,
    setTokenFn,
    setIsSignedIn,
    isSignedIn,
    setUserInfo,
    getUserInfo,
    signOutFn,
    setSignOutFn,
  ]);

  return <>{props.children}</>;
}
