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
      className="bg-transparent border-none text-blue-600 dark:text-blue-400 cursor-pointer underline hover:text-blue-700 dark:hover:text-blue-300"
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
        <div className="flex flex-col mx-auto gap-4">
          <button
            className="px-4 py-2 rounded-md font-medium transition-colors bg-blue-600 hover:bg-blue-700 text-white"
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

  return (
    <div className="flex flex-col w-full space-y-4">
      <div>
        <label className="block text-sm font-semibold text-gray-700 dark:text-gray-300 mb-2">
          Auth0 Domain
        </label>
        <input
          type="text"
          className="w-full px-3 py-2 rounded-lg border font-mono text-sm bg-white dark:bg-gray-700 text-gray-900 dark:text-gray-100 border-gray-300 dark:border-gray-600 shadow-sm focus:outline-none"
          value={domain}
          onChange={(e) => setDomain(e.target.value)}
        />
      </div>
      <div>
        <label className="block text-sm font-semibold text-gray-700 dark:text-gray-300 mb-2">
          Auth0 Client Id
        </label>
        <input
          type="text"
          className="w-full px-3 py-2 rounded-lg border font-mono text-sm bg-white dark:bg-gray-700 text-gray-900 dark:text-gray-100 border-gray-300 dark:border-gray-600 shadow-sm focus:outline-none"
          value={clientId}
          onChange={(e) => setClientId(e.target.value)}
        />
      </div>
      <div>
        <label className="block text-sm font-semibold text-gray-700 dark:text-gray-300 mb-2">
          Profile
        </label>
        <input
          type="text"
          className="w-full px-3 py-2 rounded-lg border font-mono text-sm bg-white dark:bg-gray-700 text-gray-900 dark:text-gray-100 border-gray-300 dark:border-gray-600 shadow-sm focus:outline-none"
          value={profile}
          onChange={(e) => setProfile(e.target.value)}
        />
      </div>
      <div className="flex justify-end pt-2">
        <button
          className={`px-4 py-2 rounded-md font-medium transition-colors ${
            disabledDone
              ? 'bg-gray-300 text-gray-500 cursor-not-allowed'
              : 'bg-blue-600 hover:bg-blue-700 text-white'
          }`}
          disabled={disabledDone}
          onClick={() => {
            setConfig(new Auth0Config(domain, clientId, profile));
            props.onDone();
          }}
        >
          Done
        </button>
      </div>
    </div>
  );
}

