import { useContext, useState } from "react";
import { ToolbarButton, Dialog } from "@graphiql/react";

import { AuthContext } from "../AuthContext";

import {
  SignInPanel as ClerkSignInPanel,
  UserIcon as ClerkUserIcon,
  AuthProvider as ClerkAuthProvider,
  SignOutButton as ClerkSignOutButton,
} from "./clerk";
import {
  SignInPanel as SecretSignInPanel,
  UserIcon as SecretUserIcon,
} from "./secret";

export function AuthToolbarButton() {
  const [showAuthPanel, setShowAuthPanel] = useState(false);
  const { isSignedIn } = useContext(AuthContext);

  return (
    <AuthProvider>
      <div style={{ position: "relative" }}>
        <ToolbarButton
          label="Authenticate"
          onClick={() => setShowAuthPanel(!showAuthPanel)}
        >
          <svg
            fill="none"
            viewBox="0 0 24 24"
            strokeWidth="1.5"
            stroke="currentColor"
            className="graphiql-toolbar-icon"
          >
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              d="M15.75 5.25a3 3 0 013 3m3 0a6 6 0 01-7.029 5.912c-.563-.097-1.159.026-1.563.43L10.5 17.25H8.25v2.25H6v2.25H2.25v-2.818c0-.597.237-1.17.659-1.591l6.499-6.499c.404-.404.527-1 .43-1.563A6 6 0 1121.75 8.25z"
            />
          </svg>
          {isSignedIn && (
            <div
              style={{
                position: "absolute",
                right: "0px",
                bottom: "-4px",
              }}
            >
              <UserIcon />
            </div>
          )}
        </ToolbarButton>
        <AuthPanel open={showAuthPanel} onOpenChange={setShowAuthPanel} />
      </div>
    </AuthProvider>
  );
}

export function AuthPanel(props: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const { isSignedIn } = useContext(AuthContext);

  if (!props.open) {
    return null;
  }

  if (isSignedIn) {
    return <SignOutPanel {...props} />;
  } else {
    return <SignInPanel {...props} />;
  }
}

function SignInPanel(props: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const { authenticatorInfo } = useContext(AuthContext);
  const panelStyle = authenticatorInfo?.type;

  return (
    <Dialog open={props.open} onOpenChange={props.onOpenChange}>
      <div className="graphiql-dialog-header">
        <Dialog.Title className="graphiql-dialog-title">
          Authentication
        </Dialog.Title>
        <Dialog.Close />
      </div>
      <div
        className="graphiql-dialog-section"
        style={{
          minWidth: "500px",
          alignItems: "flex-start",
        }}
      >
        {panelStyle === "clerk" ? (
          <ClerkSignInPanel />
        ) : (
          <SecretSignInPanel onDone={() => props.onOpenChange(false)} />
        )}
      </div>
    </Dialog>
  );
}

function SignOutPanel(props: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const { authenticatorInfo } = useContext(AuthContext);
  const panelStyle = authenticatorInfo?.type;

  if (panelStyle === "clerk") {
    return <ClerkSignOutButton onSignOut={() => props.onOpenChange(false)} />;
  } else {
    return <SignInPanel {...props} />;
  }
}

function UserIcon() {
  const { authenticatorInfo } = useContext(AuthContext);
  const panelStyle = authenticatorInfo?.type;

  if (panelStyle === "clerk") {
    return <ClerkUserIcon />;
  } else {
    return <SecretUserIcon />;
  }
}

function AuthProvider(props: { children: React.ReactNode }) {
  const { authenticatorInfo } = useContext(AuthContext);
  const panelStyle = authenticatorInfo?.type;

  if (panelStyle === "clerk") {
    return <ClerkAuthProvider>{props.children}</ClerkAuthProvider>;
  } else {
    return <>{props.children}</>;
  }
}
