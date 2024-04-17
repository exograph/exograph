import { useContext, useState } from "react";
import { useAuth0 } from "@auth0/auth0-react";
import { AuthConfigContext } from "./AuthConfigProvider";
import { Auth0Config } from "./Auth0Config";

export function SignInPanel() {
  const { config } = useContext(AuthConfigContext);
  const canShowSignIn = config.canSignIn();
  const { loginWithRedirect } = useAuth0();

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
      {currentPanel === "info" ? "Sign In" : "Configure Auth0"}
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
          <button
            className="graphiql-button"
            style={{
              background: "hsla(var(--color-tertiary), 1)",
              color: "white",
            }}
            onClick={() =>
              loginWithRedirect({
                // Specify custom openUrl to prevent Auth0 from reopening the window (which doesn't work in Arc)
                openUrl(url) {
                  window.location.replace(url);
                },
              })
            }
          >
            Sign in with Auth0
          </button>
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
  const [domain, setDomain] = useState<string>(config.domain || "");
  const [clientId, setClientId] = useState<string>(config.clientId || "");
  const [profile, setProfile] = useState<string>(
    config.profile || "read:current_user profile"
  );
  const disabledDone = domain === "" || clientId === "" || profile === "";
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
      <div style={labelStyle}>Auth0 Domain</div>
      <input
        style={inputStyle}
        value={domain}
        onChange={(e) => setDomain(e.target.value)}
      />
      <div style={labelStyle}>Auth0 Client Id</div>
      <input
        style={inputStyle}
        value={clientId}
        onChange={(e) => setClientId(e.target.value)}
      />
      <div style={labelStyle}>Profile</div>
      <input
        style={inputStyle}
        value={profile}
        onChange={(e) => setProfile(e.target.value)}
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
          setConfig(new Auth0Config(domain, clientId, profile));
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
