import React, {
  Dispatch,
  SetStateAction,
  useContext,
  useEffect,
  useMemo,
  useState,
} from "react";
import { AuthContext } from "../AuthContext";
import { SecretConfig } from "./SecretConfig";
import * as jose from "jose";
import { SecretAuthContext, SecretAuthProvider } from "./SecretAuthProvider";

type AuthConfig = {
  config: SecretConfig;
  setConfig: Dispatch<SetStateAction<SecretConfig>>;
};

export const AuthConfigContext = React.createContext<AuthConfig>(
  {} as AuthConfig
);

export function AuthConfigProvider(props: { children: React.ReactNode }) {
  const { plugin } = useContext(AuthContext);
  const [config, setConfig] = useState(() =>
    SecretConfig.loadConfig(plugin.config)
  );

  // Auto-save config to localStorage whenever it changes
  useEffect(() => {
    config.save();
  }, [config]);

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
  const activeProfile = useMemo(() => {
    return config.profiles.find(
      (profile) => profile.id === config.activeProfileId
    );
  }, [config]);

  useEffect(() => {
    const produceToken = async (): Promise<string | null> => {
      if (config.mode === "static") {
        return config.secret.value ? config.secret.value.trim() : null;
      }

      try {
        const parsedClaims = JSON.parse(config.claims);
        return await createJwtToken(parsedClaims, config.secret.value);
      } catch (error) {
        console.warn("Failed to create JWT token from claims:", error);
        return null;
      }
    };

    if (setTokenFn) {
      setTokenFn(signedIn ? produceToken : undefined);
    }

    if (setIsSignedIn) {
      setIsSignedIn(signedIn);
    }

    if (setUserInfo) {
      if (activeProfile) {
        setUserInfo(`${activeProfile.name} (${config.mode})`);
      } else {
        setUserInfo(undefined);
      }
    }

    if (setSignOutFn) {
      setSignOutFn(async () => {
        setSignedIn(false);
      });
    }
  }, [
    config,
    activeProfile,
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
