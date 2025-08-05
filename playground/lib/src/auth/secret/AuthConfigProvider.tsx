import React, { useContext, useEffect, useState } from "react";
import { AuthContext } from "../../AuthContext";
import { SecretConfig } from "./SecretConfig";
import * as jose from "jose";
import { SecretAuthContext, SecretAuthProvider } from "./SecretAuthProvider";

type AuthConfig = {
  config: SecretConfig;
  setConfig: (config: SecretConfig) => void;
};

export const AuthConfigContext = React.createContext<AuthConfig>(
  {} as AuthConfig
);

export function AuthConfigProvider(props: { children: React.ReactNode }) {
  const { plugin } = useContext(AuthContext);
  const [config, setConfig] = useState(SecretConfig.loadConfig(plugin.config));

  return (
    <AuthConfigContext.Provider
      value={{
        config,
        setConfig,
      }}
    >
      <SecretAuthProvider>
        <ContextInitializer>{props.children}</ContextInitializer>
      </SecretAuthProvider>
    </AuthConfigContext.Provider>
  );
}

function ContextInitializer(props: { children: React.ReactNode }) {
  const { config } = useContext(AuthConfigContext);
  const { signedIn, setSignedIn } = useContext(SecretAuthContext);
  const { setTokenFn, setIsSignedIn, setUserInfo, setSignOutFn } =
    useContext(AuthContext);

  useEffect(() => {
    const claims = config.claims;

    setTokenFn &&
      setTokenFn(
        signedIn
          ? () =>
              Promise.resolve(
                createJwtToken(JSON.parse(claims), config.secret.value)
              )
          : undefined
      );
    setIsSignedIn && setIsSignedIn(signedIn);
    setUserInfo && setUserInfo(claims);
    setSignOutFn &&
      setSignOutFn(() => {
        setSignedIn(!signedIn);
        return Promise.resolve();
      });
  }, [
    config,
    setTokenFn,
    setIsSignedIn,
    setUserInfo,
    setSignOutFn,
    setSignedIn,
    signedIn,
  ]);

  return <>{props.children}</>;
}

async function createJwtToken(
  claims: Record<string, unknown>,
  secret: string
): Promise<string | null> {
  if (secret === "") {
    return null;
  }

  const encodedSecret = new TextEncoder().encode(secret);
  const alg = "HS256";

  return await new jose.SignJWT(claims)
    .setProtectedHeader({ alg })
    .setIssuedAt()
    .setExpirationTime("10m")
    .sign(encodedSecret);
}
