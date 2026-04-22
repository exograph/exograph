import React, { useCallback, useEffect, useState } from "react";

import { Auth0Provider, useAuth0 } from "@auth0/auth0-react";
import { useAuthContext } from "../AuthContext";
import { Auth0Config } from "./Auth0Config";

type AuthConfig = {
  config: Auth0Config;
  setConfig: (config: Auth0Config) => void;
};

export const AuthConfigContext = React.createContext({} as AuthConfig);

export function AuthConfigProvider(props: { children: React.ReactNode }) {
  const [config, setConfig] = useState<Auth0Config>(Auth0Config.loadConfig());

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
      {!!config.domain && !!config.clientId ? (
        <Auth0Provider
          domain={config.domain}
          clientId={config.clientId}
          useRefreshTokens={true}
          cacheLocation="localstorage"
          authorizationParams={{
            redirect_uri: window.location.href,
            audience: `https://${config.domain}/api/v2/`,
            scope: config.profile,
          }}
        >
          <ContextInitializer>{props.children}</ContextInitializer>
        </Auth0Provider>
      ) : (
        <>{props.children}</>
      )}
    </AuthConfigContext.Provider>
  );
}

function ContextInitializer(props: { children: React.ReactNode }) {
  const { isAuthenticated, getAccessTokenSilently, user, logout } = useAuth0();
  const { setTokenFn, setIsSignedIn, setUserInfo, setSignOutFn } =
    useAuthContext();

  const signOutFn = useCallback(async () => {
    logout({ openUrl: false });
  }, [logout]);

  const getUserInfo = useCallback(() => {
    let userInfo = "";
    if (user) {
      if (user.name && user.email) {
        userInfo = `user.name (${user.email})`;
      } else if (user.name) {
        userInfo = user.name;
      } else if (user.email) {
        userInfo = user.email;
      } else {
        userInfo = "Unknown";
      }
    }
    return userInfo;
  }, [user]);

  useEffect(() => {
    setTokenFn(isAuthenticated ? () => getAccessTokenSilently() : undefined);
    setIsSignedIn(isAuthenticated);
    setUserInfo(getUserInfo());
    setSignOutFn(() => signOutFn());
  }, [
    isAuthenticated,
    setIsSignedIn,
    getAccessTokenSilently,
    setTokenFn,
    setUserInfo,
    getUserInfo,
    signOutFn,
    setSignOutFn,
  ]);

  return <>{props.children}</>;
}
