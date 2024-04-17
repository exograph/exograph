import { SignIn } from "@clerk/clerk-react";
import { useContext, useState } from "react";
import { AuthConfigContext } from "./AuthConfigProvider";
import { ClerkConfig } from "./ClerkConfig";

export function SignInPanel() {
  const { config } = useContext(AuthConfigContext);
  const canShowSignIn = config.canSignIn();

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
        <div
          style={{
            display: "flex",
            flexDirection: "column",
            marginLeft: "auto",
            marginRight: "auto",
            gap: "1rem",
          }}
        >
          <SignIn redirectUrl={"/playground"} />
          {switchLink}
        </div>
      );
    } else {
      return <ConfigurationPanel onDone={() => setCurrentPanel("sign-in")} />;
    }
  } else {
    return <ConfigurationPanel onDone={() => setCurrentPanel("sign-in")} />;
  }
}

function ConfigurationPanel(props: { onDone: () => void }) {
  const { config, setConfig } = useContext(AuthConfigContext);
  const [publishableKey, setPublishableKey] = useState<string>(
    config.publishableKey || ""
  );
  const [templateId, setTemplateId] = useState<string>(config.templateId || "");
  const disabledDone = publishableKey === "";
  const background = disabledDone
    ? "hsla(var(--color-secondary), 0.5)"
    : "hsla(var(--color-secondary), 1)";

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
          background,
          color: "white",
          width: "fit-content",
          alignSelf: "flex-end",
          marginTop: "1rem",
        }}
        disabled={disabledDone}
        onClick={() => {
          setConfig(new ClerkConfig(publishableKey, templateId));
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
  background: "transparent",
  borderRadius: "10px",
  marginBottom: "10px",
  padding: "10px",
  border: "1px solid hsla(var(--color-neutral), 0.2)",
  boxShadow: "0px 0px 8px 0px hsla(var(--color-neutral), 0.2)",
};
