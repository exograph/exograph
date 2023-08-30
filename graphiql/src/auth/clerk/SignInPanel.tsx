import { SignIn } from "@clerk/clerk-react";
import { AuthContext, ClerkAuthenticatorInfo } from "../../AuthContext";
import { useContext, useState } from "react";

export function SignInPanel() {
  const { authenticatorInfo } = useContext(AuthContext);

  const canShowSignIn =
    authenticatorInfo &&
    authenticatorInfo.type === "clerk" &&
    authenticatorInfo.publishableKey;

  const [currentPanel, setCurrentPanel] = useState<"info" | "sign-in">(
    canShowSignIn ? "sign-in" : "info"
  );

  const switchLink = (
    <button
      style={{
        background: "none",
        border: "none",
        color: "hsl(var(--color-primary))",
        cursor: "pointer",
        textDecoration: "underline",
      }}
      onClick={(e) => {
        e.preventDefault();
        setCurrentPanel(currentPanel === "info" ? "sign-in" : "info");
      }}
    >
      {currentPanel === "info" ? "Sign In" : "Configure Clerk"}
    </button>
  );

  if (canShowSignIn) {
    if (currentPanel === "sign-in") {
      return (
        <div style={{ display: "flex", flexDirection: "column", marginLeft: "auto", marginRight: "auto", gap: "1rem" }}>
          <SignIn redirectUrl={"/playground"}/>
          {switchLink}
        </div>
      );
    } else {
      return <ClerkInfo onDone={() => setCurrentPanel("sign-in")} />;
    }
  } else {
    return <ClerkInfo onDone={() => setCurrentPanel("sign-in")} />;
  }
}

function ClerkInfo(props: { onDone: () => void }) {
  const { authenticatorInfo, setAuthenticatorInfo } = useContext(AuthContext);
  const clerkAuthenticatorInfo = authenticatorInfo as ClerkAuthenticatorInfo;

  const [publishableKey, setPublishableKey] = useState<string>(
    clerkAuthenticatorInfo.publishableKey || ""
  );
  const [templateId, setTemplateId] = useState<string>(
    clerkAuthenticatorInfo.templateId || ""
  );

  return (
    <div
      style={{
        display: "flex",
        flexDirection: "column",
        width: "100%",
      }}
    >
      <div style={labelStyle}>Clerk Publishable Key</div>
      <input
        style={inputStyle}
        value={publishableKey}
        onChange={(e) => setPublishableKey(e.target.value)}
      />
      <div style={labelStyle}>Template for getting token (optional)</div>
      <input
        style={inputStyle}
        value={templateId}
        onChange={(e) => setTemplateId(e.target.value)}
      />
      <button
        className="graphiql-button"
        style={{
          background: "hsla(var(--color-secondary), 1)",
          color: "white",
          width: "fit-content",
          alignSelf: "flex-end",
          marginTop: "1rem",
        }}
        onClick={() => {
          setAuthenticatorInfo &&
            setAuthenticatorInfo({
              type: "clerk",
              publishableKey,
              templateId,
            });

          props.onDone();
        }}
      >
        Done
      </button>
    </div>
  );
}

const labelStyle = {
  fontSize: "var(--font-size-h4)",
  fontWeight: "bold",
  marginTop: "0.5rem",
  marginBottom: "0.4rem",
};

const inputStyle = {
  borderRadius:"10px",
  marginBottom: "10px", 
  padding: "10px",
  border: "1px solid hsla(var(--color-neutral), 0.2)",
  boxShadow: "0px 0px 8px 0px hsla(var(--color-neutral), 0.2)"
}
