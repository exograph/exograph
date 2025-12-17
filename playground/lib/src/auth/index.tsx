import { forwardRef, useContext, useEffect, useRef, useState } from "react";

import { AuthContext } from "./AuthContext";

export function AuthToolbarButton() {
  const [showAuthPanel, setShowAuthPanel] = useState(false);
  const { plugin, isSignedIn, userInfo } = useContext(AuthContext);

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
    <div className="relative">
      <button
        title={getTooltip()}
        onClick={() => setShowAuthPanel(!showAuthPanel)}
        className="relative flex items-center justify-center w-8 h-8 rounded-md border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-800 hover:bg-gray-50 dark:hover:bg-gray-700 text-gray-700 dark:text-gray-300 transition-colors"
      >
        <svg
          fill="none"
          viewBox="0 0 24 24"
          strokeWidth="1.5"
          stroke="currentColor"
          className="w-4 h-4"
        >
          <path
            strokeLinecap="round"
            strokeLinejoin="round"
            d="M15.75 5.25a3 3 0 013 3m3 0a6 6 0 01-7.029 5.912c-.563-.097-1.159.026-1.563.43L10.5 17.25H8.25v2.25H6v2.25H2.25v-2.818c0-.597.237-1.17.659-1.591l6.499-6.499c.404-.404.527-1 .43-1.563A6 6 0 1121.75 8.25z"
          />
        </svg>
        <div className="absolute -right-1 -bottom-1">{getUserIcon()}</div>
      </button>
      <AuthPanel open={showAuthPanel} onOpenChange={setShowAuthPanel} />
    </div>
  );
}

export function AuthPanel(props: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const { isSignedIn } = useContext(AuthContext);
  const signoutRef = { current: null as HTMLButtonElement | null };

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
    return (
      <SignOutButton
        onSignOut={() => props.onOpenChange(false)}
        ref={signoutRef}
      />
    );
  } else {
    return <SignInModal {...props} />;
  }
}

function SignInModal(props: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const { plugin } = useContext(AuthContext);
  const SignInPanel = plugin.getSignInPanel();
  const dialogRef = useRef<HTMLDialogElement>(null);

  useEffect(() => {
    const dialog = dialogRef.current;
    if (!dialog) return;

    if (props.open) {
      dialog.showModal();
    } else {
      dialog.close();
    }
  }, [props.open]);

  const handleDialogClick = (e: React.MouseEvent<HTMLDialogElement>) => {
    if (e.target === e.currentTarget) {
      props.onOpenChange(false);
    }
  };

  return (
    <dialog
      ref={dialogRef}
      className="bg-white dark:bg-gray-800 rounded-lg shadow-lg border border-gray-200 dark:border-gray-700 min-w-[500px] max-w-2xl"
      onClose={() => props.onOpenChange(false)}
      onClick={handleDialogClick}
    >
      {/* Header */}
      <div className="flex items-center justify-between p-4 border-b border-gray-200 dark:border-gray-700">
        <h2 className="text-lg font-semibold text-gray-900 dark:text-gray-100">
          Authentication
        </h2>
        <button
          onClick={() => props.onOpenChange(false)}
          className="p-1 rounded-md hover:bg-gray-100 dark:hover:bg-gray-700 text-gray-500 dark:text-gray-400"
        >
          <svg
            className="w-5 h-5"
            fill="none"
            viewBox="0 0 24 24"
            stroke="currentColor"
          >
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              strokeWidth={2}
              d="M6 18L18 6M6 6l12 12"
            />
          </svg>
        </button>
      </div>

      {/* Content */}
      <div className="p-4">
        <SignInPanel onDone={() => props.onOpenChange(false)} />
      </div>
    </dialog>
  );
}

const SignOutButton = forwardRef<HTMLButtonElement, { onSignOut: () => void }>(
  (props, ref) => {
    const { getSignOutFn } = useContext(AuthContext);

    return (
      <button
        ref={ref}
        className="absolute right-0 top-10 bg-blue-600 hover:bg-blue-700 text-white px-4 py-2 rounded-md text-sm font-medium transition-colors z-50 shadow-lg border border-blue-700"
        onClick={() => {
          getSignOutFn && getSignOutFn().then(props.onSignOut);
        }}
      >
        Sign Out
      </button>
    );
  }
);
