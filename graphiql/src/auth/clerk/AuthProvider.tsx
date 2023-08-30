import { ClerkProvider, useAuth, useUser } from "@clerk/clerk-react";
import { useContext, useEffect } from "react";
import { AuthContext, ClerkAuthenticatorInfo } from "../../AuthContext";

export function AuthProvider(props: { children: React.ReactNode }) {
  const { authenticatorInfo } = useContext(AuthContext);

  const showSignIn =
    authenticatorInfo &&
    authenticatorInfo.type === "clerk" &&
    authenticatorInfo.publishableKey !== undefined;

  if (showSignIn && authenticatorInfo.publishableKey) {
    return (
      <ClerkProvider publishableKey={authenticatorInfo.publishableKey}>
        <ContextInitializer>{props.children}</ContextInitializer>
      </ClerkProvider>
    );
  } else {
    return <>{props.children}</>;
  }
}

function ContextInitializer(props: { children: React.ReactNode }) {
  const { isSignedIn } = useUser();
  const { getToken, signOut } = useAuth();
  const { setTokenFn, setIsSignedIn, authenticatorInfo } =
    useContext(AuthContext);

  useEffect(() => {
    let templateId = (authenticatorInfo as ClerkAuthenticatorInfo).templateId;
    const getTokenOptions = templateId ? { template: templateId } : undefined;
    setTokenFn &&
      setTokenFn(isSignedIn ? () => getToken(getTokenOptions) : undefined);
    setIsSignedIn && setIsSignedIn(isSignedIn);
  }, [
    isSignedIn,
    setIsSignedIn,
    getToken,
    setTokenFn,
    signOut,
    authenticatorInfo,
  ]);

  return <>{props.children}</>;
}
