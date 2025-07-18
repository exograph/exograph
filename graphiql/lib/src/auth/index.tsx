import React, { forwardRef, useContext, useEffect, useState } from "react";
import { ToolbarButton, Dialog } from "@graphiql/react";

import { AuthContext } from "../AuthContext";

export function AuthToolbarButton() {
  const [showAuthPanel, setShowAuthPanel] = useState(false);
  const { plugin, isSignedIn, userInfo } = useContext(AuthContext);
  const AuthConfigProvider = plugin.getAuthConfigProvider();

  const getUserIcon = () => {
    if (isSignedIn) {
      const UserIcon = plugin.getUserIcon();
      return <UserIcon />;
    } else {
      return null;
    }
  };

  const getTooltip = () => {
    if (isSignedIn) {
      if (userInfo) {
        return `${userInfo}`;
      } else {
        return "Sign out";
      }
    } else {
      return "Authenticate";
    }
  };

  return (
    <AuthConfigProvider>
      <div style={{ position: "relative" }}>
        <ToolbarButton label={getTooltip()} onClick={() => setShowAuthPanel(!showAuthPanel)}>
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
          <div
            style={{
              position: "absolute",
              right: "0px",
              bottom: "-4px",
            }}
          >
            {getUserIcon()}
          </div>
        </ToolbarButton>
        <AuthPanel open={showAuthPanel} onOpenChange={setShowAuthPanel} />
      </div>
    </AuthConfigProvider>
  );
}

export function AuthPanel(props: { open: boolean; onOpenChange: (open: boolean) => void }) {
  const { isSignedIn } = useContext(AuthContext);
  const signoutRef = React.createRef<HTMLButtonElement>();

  useEffect(() => {
    const handleMouseDown = (e: MouseEvent) => {
      if (e.target && signoutRef.current && e.target !== signoutRef.current) {
        props.onOpenChange(false);
      }
    };
    document.addEventListener("mousedown", handleMouseDown);
    return () => document.removeEventListener("mousedown", handleMouseDown);
  }, [props, signoutRef]);

  if (!props.open) {
    return null;
  }

  if (isSignedIn) {
    return <SignOutButton onSignOut={() => props.onOpenChange(false)} ref={signoutRef} />;
  } else {
    return <SignInDialog {...props} />;
  }
}

function SignInDialog(props: { open: boolean; onOpenChange: (open: boolean) => void }) {
  const { plugin } = useContext(AuthContext);
  const SignInPanel = plugin.getSignInPanel();

  return (
    <Dialog modal={false} open={props.open} onOpenChange={props.onOpenChange}>
      <div className="graphiql-dialog-header">
        <Dialog.Title className="graphiql-dialog-title">Authentication</Dialog.Title>
        <Dialog.Close />
      </div>
      <div
        className="graphiql-dialog-section"
        style={{
          minWidth: "500px",
          alignItems: "flex-start",
        }}
      >
        <SignInPanel onDone={() => props.onOpenChange(false)} />
      </div>
    </Dialog>
  );
}

const SignOutButton = forwardRef<HTMLButtonElement, { onSignOut: () => void }>((props, ref) => {
  const { getSignOutFn } = useContext(AuthContext);

  return (
    <button
      ref={ref}
      style={{
        background: "hsla(var(--color-secondary), 1)",
        color: "white",
        width: "100px",
        padding: "5px",
        borderRadius: "5px",
        position: "absolute",
        right: "-16px",
        marginTop: "5px",
        zIndex: 100,
      }}
      onClick={() => {
        getSignOutFn && getSignOutFn().then(props.onSignOut);
      }}
    >
      Sign Out
    </button>
  );
});
