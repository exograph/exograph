import { useContext, useMemo } from "react";

import { AuthConfigContext } from "./AuthConfigProvider";
import { SecretAuthContext } from "./SecretAuthProvider";
import { AuthProfileMode } from "./SecretConfig";

type RoleInfo = {
  availableRoles: string[];
  defaultRole: string;
};

function decodeRolesFromClaims(claims: string): RoleInfo | null {
  try {
    const parsed = JSON.parse(claims);
    const hasuraClaims =
      parsed["https://hasura.io/jwt/claims"] || parsed["claims.jwt.hasura.io"];
    if (hasuraClaims) {
      return {
        availableRoles: hasuraClaims["x-hasura-allowed-roles"] || [],
        defaultRole: hasuraClaims["x-hasura-default-role"] || "",
      };
    }
  } catch {
    // ignore parse errors
  }
  return null;
}

function decodeRolesFromToken(token: string): RoleInfo | null {
  if (!token) {
    return null;
  }

  const parts = token.split(".");
  if (parts.length < 2) {
    return null;
  }

  try {
    const padded = parts[1].replace(/-/g, "+").replace(/_/g, "/");
    const json = atob(padded);
    return decodeRolesFromClaims(json);
  } catch {
    return null;
  }
}

export function SignInPanel(props: { onDone: () => void }) {
  const { config, setConfig } = useContext(AuthConfigContext);
  const { signedIn, setSignedIn } = useContext(SecretAuthContext);

  const activeProfile = useMemo(
    () =>
      config.profiles.find(
        (profile) => profile.id === config.activeProfileId
      ),
    [config]
  );

  const roleInfo =
    config.mode === "static"
      ? decodeRolesFromToken(config.secret.value)
      : decodeRolesFromClaims(config.claims);

  const activeRole =
    config.headers["x-hasura-role"] || roleInfo?.defaultRole || "";

  const modeLabel: Record<AuthProfileMode, string> = {
    static: "Static JWT token",
    generated: "Generated token (HS256 secret + claims)",
  };

  return (
    <div className="flex flex-col w-full space-y-4">
      <div>
        <label className="block text-sm font-semibold text-gray-700 dark:text-gray-300 mb-2">
          Profile
        </label>
        <select
          className="w-full px-3 py-2 rounded-lg border bg-white dark:bg-gray-700 text-gray-900 dark:text-gray-100 border-gray-300 dark:border-gray-600 shadow-sm focus:outline-none"
          value={config.activeProfileId}
          onChange={(event) =>
            setConfig((current) =>
              current.withActiveProfile(event.target.value)
            )
          }
        >
          {config.profiles.map((profile) => (
            <option key={profile.id} value={profile.id}>
              {profile.name}
            </option>
          ))}
        </select>
      </div>

      <div>
        <label className="block text-sm font-semibold text-gray-700 dark:text-gray-300 mb-1">
          Mode
        </label>
        <p className="text-sm text-gray-600 dark:text-gray-300">
          {modeLabel[config.mode]}
        </p>
        <p className="text-xs text-gray-500 dark:text-gray-400 mt-1">
          Use the <span className="font-semibold">Auth</span> tab to edit profile details.
        </p>
      </div>

      {roleInfo && (
        <div className="bg-blue-50 dark:bg-blue-900/20 rounded-lg p-3 space-y-2">
          <div className="text-sm">
            <span className="font-semibold text-gray-700 dark:text-gray-300">
              Available Roles:{" "}
            </span>
            <span className="text-gray-600 dark:text-gray-400">
              {roleInfo.availableRoles.join(", ") || "None"}
            </span>
          </div>
          <div className="text-sm">
            <span className="font-semibold text-gray-700 dark:text-gray-300">
              Default Role:{" "}
            </span>
            <span className="text-gray-600 dark:text-gray-400">
              {roleInfo.defaultRole || "None"}
            </span>
          </div>
          <div className="text-sm">
            <span className="font-semibold text-gray-700 dark:text-gray-300">
              Active Role:{" "}
            </span>
            <span className="font-mono text-blue-600 dark:text-blue-400">
              {activeRole || "None"}
            </span>
          </div>
        </div>
      )}

      {activeProfile?.headers && Object.keys(activeProfile.headers).length > 0 && (
        <div className="text-sm text-gray-600 dark:text-gray-300 border border-gray-200 dark:border-gray-700 rounded-lg p-3">
          <h3 className="font-semibold text-gray-700 dark:text-gray-200 mb-2">
            Custom Headers
          </h3>
          <dl className="space-y-1">
            {Object.entries(activeProfile.headers).map(([key, value]) => (
              <div key={key} className="flex items-center justify-between">
                <dt className="font-medium text-gray-700 dark:text-gray-300">
                  {key}
                </dt>
                <dd className="font-mono text-xs text-gray-600 dark:text-gray-400 overflow-hidden text-ellipsis max-w-[60%]">
                  {value}
                </dd>
              </div>
            ))}
          </dl>
        </div>
      )}

      <div className="flex justify-end pt-2 gap-2">
        {signedIn ? (
          <button
            className="px-4 py-2 rounded-md font-medium transition-colors bg-gray-200 hover:bg-gray-300 dark:bg-gray-700 dark:hover:bg-gray-600 text-gray-700 dark:text-gray-200"
            onClick={() => {
              setSignedIn(false);
              props.onDone();
            }}
          >
            Sign Out
          </button>
        ) : (
          <button
            className={`px-4 py-2 rounded-md font-medium transition-colors ${
              config.canSignIn()
                ? "bg-blue-600 hover:bg-blue-700 text-white"
                : "bg-gray-300 text-gray-500 cursor-not-allowed"
            }`}
            onClick={() => {
              if (config.canSignIn()) {
                setSignedIn(true);
                props.onDone();
              }
            }}
            disabled={!config.canSignIn()}
          >
            Sign In
          </button>
        )}
      </div>
    </div>
  );
}
