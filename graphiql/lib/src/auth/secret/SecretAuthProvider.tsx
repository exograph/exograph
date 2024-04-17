import React from "react";

type SecretAuth = {
  signedIn: boolean;
  setSignedIn: (signedIn: boolean) => void;
};

export const SecretAuthContext = React.createContext<SecretAuth>(
  {} as SecretAuth
);

export function SecretAuthProvider(props: { children: React.ReactNode }) {
  const [signedIn, setSignedIn] = React.useState(false);

  return (
    <SecretAuthContext.Provider value={{ signedIn, setSignedIn }}>
      {props.children}
    </SecretAuthContext.Provider>
  );
}
