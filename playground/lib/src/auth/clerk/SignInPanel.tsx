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
      className="bg-transparent border-none text-blue-600 dark:text-blue-400 cursor-pointer underline hover:text-blue-700 dark:hover:text-blue-300"
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
        <div className="flex flex-col mx-auto gap-4">
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

  return (
    <div className="flex flex-col w-full space-y-4">
      <div>
        <label className="block text-sm font-semibold text-gray-700 dark:text-gray-300 mb-2">
          Clerk Publishable Key
        </label>
        <input
          type="text"
          className="w-full px-3 py-2 rounded-lg border font-mono text-sm bg-white dark:bg-gray-700 text-gray-900 dark:text-gray-100 border-gray-300 dark:border-gray-600 shadow-sm focus:outline-none"
          value={publishableKey}
          onChange={(e) => setPublishableKey(e.target.value)}
        />
      </div>
      <div>
        <label className="block text-sm font-semibold text-gray-700 dark:text-gray-300 mb-2">
          Template for getting token (optional)
        </label>
        <input
          type="text"
          className="w-full px-3 py-2 rounded-lg border font-mono text-sm bg-white dark:bg-gray-700 text-gray-900 dark:text-gray-100 border-gray-300 dark:border-gray-600 shadow-sm focus:outline-none"
          value={templateId}
          onChange={(e) => setTemplateId(e.target.value)}
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
            setConfig(new ClerkConfig(publishableKey, templateId));
            props.onDone();
          }}
        >
          Done
        </button>
      </div>
    </div>
  );
}

