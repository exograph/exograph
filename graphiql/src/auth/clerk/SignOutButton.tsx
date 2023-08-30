import { useAuth } from "@clerk/clerk-react";

export function SignOutButton(props: { onSignOut: () => void }) {
  const { signOut } = useAuth();

  return (
    <button
      style={{
        background: "hsla(var(--color-secondary), 1)",
        color: "white",
        width: "100px",
        padding: "5px",
        borderRadius: "5px",
        position: "absolute",
        right: 0,
        zIndex: 100,
      }}
      onClick={() => {
        signOut &&
          signOut().then(() => {
            props.onSignOut();
          });
      }}
    >
      Sign Out
    </button>
  );
}
