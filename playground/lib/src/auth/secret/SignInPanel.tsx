import { useContext, useEffect, useState } from "react";
import Editor from "@monaco-editor/react";
import { useTheme } from "../../util/theme";

import { AuthConfigContext } from "./AuthConfigProvider";
import { SecretAuthContext } from "./SecretAuthProvider";

export function SignInPanel(props: { onDone: () => void }) {
  const { config, setConfig } = useContext(AuthConfigContext);
  const { setSignedIn } = useContext(SecretAuthContext);
  const theme = useTheme();

  const [jwtSecret, setJwtSecret] = useState(config.secret.value);
  const [claims, setClaims] = useState(config.claims || "");
  const [claimsError, setClaimsError] = useState<string | undefined>(undefined);

  useEffect(() => {
    try {
      JSON.parse(claims);
      setClaimsError(undefined);
    } catch (e) {
      setClaimsError((e as Error).message);
      return;
    }
  }, [claims]);

  const enableSignIn = !claimsError && jwtSecret && claims ? true : false;

  function onSignIn() {
    setConfig(config.updated(jwtSecret, claims));
    setSignedIn(true);
    props.onDone();
  }

  return (
    <div className="flex flex-col w-full space-y-4">
      <div>
        <label className="block text-sm font-semibold text-gray-700 dark:text-gray-300 mb-2">
          Secret
        </label>
        <input
          type="text"
          className={`w-full px-3 py-2 rounded-lg border font-mono text-sm ${
            config.secret.readOnly
              ? "bg-gray-100 dark:bg-gray-700 cursor-not-allowed text-gray-500 dark:text-gray-400"
              : "bg-white dark:bg-gray-700 text-gray-900 dark:text-gray-100"
          } border-gray-300 dark:border-gray-600 shadow-sm focus:outline-none`}
          placeholder="EXO_JWT_SECRET value"
          value={jwtSecret}
          readOnly={config.secret.readOnly}
          onChange={(e) => setJwtSecret(e.target.value)}
        />
      </div>

      <div>
        <label className="block text-sm font-semibold text-gray-700 dark:text-gray-300 mb-2">
          Claims
        </label>
        <div className="rounded-lg border border-gray-300 dark:border-gray-600 shadow-sm overflow-hidden">
          <Editor
            height="5rem"
            defaultLanguage="json"
            value={claims}
            onChange={(value) => setClaims(value || "")}
            theme={theme === "dark" ? "vs-dark" : "vs"}
            options={{
              minimap: { enabled: false },
              lineNumbers: "off",
              folding: false,
              glyphMargin: false,
              lineDecorationsWidth: 0,
              lineNumbersMinChars: 0,
              scrollBeyondLastLine: false,
              automaticLayout: true,
              fontSize: 14,
              fontFamily:
                "ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace",
              scrollbar: {
                vertical: "hidden",
                horizontal: "hidden",
              },
            }}
          />
        </div>
        {claimsError && (
          <div className="text-red-600 text-sm mt-1 min-h-[1.5rem]">
            {claimsError}
          </div>
        )}
      </div>

      <div className="flex justify-end pt-2">
        <button
          className={`px-4 py-2 rounded-md font-medium transition-colors ${
            enableSignIn
              ? "bg-blue-600 hover:bg-blue-700 text-white"
              : "bg-gray-300 text-gray-500 cursor-not-allowed"
          }`}
          onClick={() => {
            onSignIn();
          }}
          disabled={!enableSignIn}
        >
          Sign In
        </button>
      </div>
    </div>
  );
}
