import {
  Dispatch,
  SetStateAction,
  useContext,
  useMemo,
  useState,
} from "react";
import Editor from "@monaco-editor/react";

import { AuthConfigContext } from "./secret/AuthConfigProvider";
import { SecretAuthContext } from "./secret/SecretAuthProvider";
import {
  AuthProfile,
  AuthProfileMode,
  SecretConfig,
} from "./secret/SecretConfig";
import { useTheme } from "../util/theme";

const modeLabels: Record<AuthProfileMode, string> = {
  generated: "Sign with shared secret (HS256)",
  static: "Use pre-generated JWT",
};

type RoleInfo = {
  availableRoles: string[];
  defaultRole: string;
};

type JwtDetails = {
  header: Record<string, unknown> | null;
  payload: Record<string, unknown> | null;
  signature: string | null;
  error?: string;
};

type JwtMetadata = {
  issuer?: string;
  audience?: string;
  subject?: string;
  issuedAt?: string;
  expiresAt?: string;
  notBefore?: string;
  tokenStatus?: string;
  jwtId?: string;
  algorithm?: string;
  keyId?: string;
  tokenType?: string;
};

function parseJsonObject(source: string): Record<string, unknown> {
  const parsed = JSON.parse(source);
  if (
    parsed !== null &&
    typeof parsed === "object" &&
    !Array.isArray(parsed)
  ) {
    return parsed as Record<string, unknown>;
  }
  throw new Error("Expected JSON object");
}

function decodeBase64Url(segment: string): string {
  const normalized = segment.replace(/-/g, "+").replace(/_/g, "/");
  const padding = normalized.length % 4;
  const padded =
    padding === 0 ? normalized : normalized + "=".repeat(4 - padding);
  const binary = atob(padded);
  if (typeof TextDecoder !== "undefined") {
    try {
      const bytes = Uint8Array.from(binary, (char) => char.charCodeAt(0));
      return new TextDecoder().decode(bytes);
    } catch {
      // Fall back to binary string below
    }
  }
  return binary;
}

function decodeJwt(token: string): JwtDetails | null {
  if (!token || !token.trim()) {
    return null;
  }

  const parts = token.split(".");
  if (parts.length < 2) {
    return {
      header: null,
      payload: null,
      signature: null,
      error: "Token must include header and payload sections.",
    };
  }

  const [rawHeader, rawPayload, rawSignature] = parts;

  let header: Record<string, unknown> | null = null;
  let payload: Record<string, unknown> | null = null;
  const errors: string[] = [];

  if (rawHeader) {
    try {
      header = parseJsonObject(decodeBase64Url(rawHeader));
    } catch (error) {
      errors.push(`header (${(error as Error).message})`);
    }
  }

  if (rawPayload) {
    try {
      payload = parseJsonObject(decodeBase64Url(rawPayload));
    } catch (error) {
      errors.push(`payload (${(error as Error).message})`);
    }
  }

  return {
    header,
    payload,
    signature: parts.length > 2 ? rawSignature ?? null : null,
    error:
      errors.length > 0
        ? `Unable to parse JWT ${errors.join(" and ")}.`
        : undefined,
  };
}

function asNumber(value: unknown): number | undefined {
  if (typeof value === "number" && Number.isFinite(value)) {
    return value;
  }
  if (typeof value === "string" && value.trim() !== "") {
    const parsed = Number(value);
    return Number.isFinite(parsed) ? parsed : undefined;
  }
  return undefined;
}

function numericToMillis(value: number): number {
  return value > 1e12 ? value : value * 1000;
}

function formatTimestamp(value: unknown): string | undefined {
  const numeric = asNumber(value);
  if (numeric === undefined) {
    return undefined;
  }
  const millis = numericToMillis(numeric);
  const date = new Date(millis);
  if (Number.isNaN(date.getTime())) {
    return undefined;
  }
  const epochUnit = numeric > 1e12 ? "ms" : "s";
  return `${date.toLocaleString()} (epoch ${numeric}${epochUnit})`;
}

function formatAudience(value: unknown): string | undefined {
  if (Array.isArray(value)) {
    const entries = value.map((item) =>
      typeof item === "string" ? item : JSON.stringify(item)
    );
    return entries.join(", ");
  }
  if (typeof value === "string") {
    return value;
  }
  return undefined;
}

function computeStatus(payload: Record<string, unknown> | null): string | undefined {
  if (!payload) {
    return undefined;
  }
  const now = Date.now();
  const notBefore = asNumber(payload["nbf"]);
  if (notBefore !== undefined) {
    const nbfMillis = numericToMillis(notBefore);
    if (now < nbfMillis) {
      return `Not valid before ${new Date(nbfMillis).toLocaleString()}`;
    }
  }
  const expires = asNumber(payload["exp"]);
  if (expires !== undefined) {
    const expMillis = numericToMillis(expires);
    if (now >= expMillis) {
      return `Expired at ${new Date(expMillis).toLocaleString()}`;
    }
    return `Active (expires ${new Date(expMillis).toLocaleString()})`;
  }
  return "Active";
}

function extractJwtMetadata(
  header: Record<string, unknown> | null,
  payload: Record<string, unknown> | null
): JwtMetadata {
  return {
    issuer:
      payload && typeof payload["iss"] === "string"
        ? (payload["iss"] as string)
        : undefined,
    audience: payload ? formatAudience(payload["aud"]) : undefined,
    subject:
      payload && typeof payload["sub"] === "string"
        ? (payload["sub"] as string)
        : undefined,
    issuedAt: formatTimestamp(payload ? payload["iat"] : undefined),
    expiresAt: formatTimestamp(payload ? payload["exp"] : undefined),
    notBefore: formatTimestamp(payload ? payload["nbf"] : undefined),
    tokenStatus: computeStatus(payload),
    jwtId:
      payload && typeof payload["jti"] === "string"
        ? (payload["jti"] as string)
        : undefined,
    algorithm:
      header && typeof header["alg"] === "string"
        ? (header["alg"] as string)
        : undefined,
    keyId:
      header && typeof header["kid"] === "string"
        ? (header["kid"] as string)
        : undefined,
    tokenType:
      header && typeof header["typ"] === "string"
        ? (header["typ"] as string)
        : undefined,
  };
}

function stringifyJson(value: Record<string, unknown> | null): string {
  if (!value) {
    return "";
  }
  return JSON.stringify(value, null, 2);
}

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

function nextProfileName(existing: AuthProfile[]): string {
  const base = "Profile";
  const suffix = existing.length + 1;
  return `${base} ${suffix}`;
}

function updateHeaders(
  setConfig: Dispatch<SetStateAction<SecretConfig>>,
  mapper: (headers: Record<string, string>) => Record<string, string>
) {
  setConfig((current) => current.updateHeaders(mapper(current.headers)));
}

export function AuthProfilesTab() {
  const { config, setConfig } = useContext(AuthConfigContext);
  const { signedIn, setSignedIn } = useContext(SecretAuthContext);
  const theme = useTheme();

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

  const [claimsError, setClaimsError] = useState<string | null>(null);
  const [newHeaderKey, setNewHeaderKey] = useState("");
  const [newHeaderValue, setNewHeaderValue] = useState("");

  const headerEntries = useMemo(
    () => Object.entries(config.headers),
    [config]
  );

  const canSignIn = config.canSignIn();

  const jwtDetails = useMemo<JwtDetails | null>(() => {
    if (config.mode === "static") {
      return decodeJwt(config.secret.value);
    }
    try {
      const parsedClaims = parseJsonObject(config.claims || "{}");
      return {
        header: { alg: "HS256", typ: "JWT" },
        payload: parsedClaims,
        signature: null,
        error: undefined,
      };
    } catch (error) {
      return {
        header: null,
        payload: null,
        signature: null,
        error: `Invalid claims JSON: ${(error as Error).message}`,
      };
    }
  }, [config]);

  const jwtMetadata = useMemo<JwtMetadata | null>(() => {
    if (!jwtDetails || jwtDetails.error) {
      return null;
    }
    return extractJwtMetadata(jwtDetails.header, jwtDetails.payload);
  }, [jwtDetails]);

  const summaryEntries = useMemo(
    () =>
      jwtMetadata
        ? [
            { label: "Issuer", value: jwtMetadata.issuer },
            { label: "Audience", value: jwtMetadata.audience },
            { label: "Subject", value: jwtMetadata.subject },
            { label: "Status", value: jwtMetadata.tokenStatus },
            { label: "Issued At", value: jwtMetadata.issuedAt },
            { label: "Not Before", value: jwtMetadata.notBefore },
            { label: "Expires At", value: jwtMetadata.expiresAt },
            { label: "JWT ID", value: jwtMetadata.jwtId },
            { label: "Algorithm", value: jwtMetadata.algorithm },
            { label: "Token Type", value: jwtMetadata.tokenType },
            { label: "Key ID", value: jwtMetadata.keyId },
          ].filter((entry) => entry.value && entry.value !== "")
        : [],
    [jwtMetadata]
  );

  return (
    <div className="h-full flex">
      <aside className="w-64 border-r border-gray-200 dark:border-gray-700 p-4 space-y-4">
        <div className="flex items-center justify-between">
          <h2 className="text-sm font-semibold text-gray-700 dark:text-gray-200">
            Profiles
          </h2>
          <button
            className="text-sm text-blue-600 hover:text-blue-700 dark:text-blue-400 dark:hover:text-blue-300"
            onClick={() => {
              setConfig((current) =>
                current.addProfile({
                  name: nextProfileName(current.profiles),
                  mode: "static",
                })
              );
            }}
          >
            + Add
          </button>
        </div>
        <ul className="space-y-1">
          {config.profiles.map((profile) => {
            const isActive = profile.id === config.activeProfileId;
            return (
              <li key={profile.id}>
                <button
                  className={`w-full text-left px-3 py-2 rounded-md text-sm transition-colors ${
                    isActive
                      ? "bg-blue-100 dark:bg-blue-900/40 text-blue-700 dark:text-blue-300"
                      : "hover:bg-gray-100 dark:hover:bg-gray-800 text-gray-700 dark:text-gray-200"
                  }`}
                  onClick={() =>
                    setConfig((current) => current.withActiveProfile(profile.id))
                  }
                >
                  <div className="font-medium">{profile.name}</div>
                  <div className="text-xs text-gray-500 dark:text-gray-400">
                    {modeLabels[profile.mode]}
                  </div>
                </button>
              </li>
            );
          })}
        </ul>
        <button
          className="text-xs text-red-600 hover:text-red-700 dark:text-red-400 dark:hover:text-red-300 disabled:opacity-50 disabled:cursor-not-allowed"
          disabled={config.profiles.length <= 1}
          onClick={() =>
            activeProfile &&
            setConfig((current) => current.removeProfile(activeProfile.id))
          }
        >
          Delete selected profile
        </button>
      </aside>

      <main className="flex-1 overflow-y-auto p-6 space-y-6">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-3">
            <input
              type="text"
              className="px-3 py-2 rounded-lg border bg-white dark:bg-gray-800 text-gray-900 dark:text-gray-100 border-gray-300 dark:border-gray-600 shadow-sm focus:outline-none text-lg font-semibold"
              value={activeProfile?.name ?? ""}
              onChange={(event) =>
                setConfig((current) =>
                  current.renameActiveProfile(event.target.value)
                )
              }
            />
            <select
              className="px-3 py-2 rounded-lg border bg-white dark:bg-gray-800 text-gray-900 dark:text-gray-100 border-gray-300 dark:border-gray-600 shadow-sm focus:outline-none text-sm"
              value={config.mode}
              onChange={(event) =>
                setConfig((current) =>
                  current.setActiveMode(event.target.value as AuthProfileMode)
                )
              }
            >
              <option value="static">Use pre-generated JWT</option>
              <option value="generated">Generate with HS256 secret</option>
            </select>
          </div>
          <div className="flex items-center gap-2">
            <span
              className={`text-sm ${
                signedIn
                  ? "text-green-600 dark:text-green-400"
                  : "text-gray-500 dark:text-gray-400"
              }`}
            >
              {signedIn ? "Signed in" : "Signed out"}
            </span>
            {signedIn ? (
              <button
                className="px-4 py-2 rounded-md font-medium transition-colors bg-gray-200 hover:bg-gray-300 dark:bg-gray-700 dark:hover:bg-gray-600 text-gray-700 dark:text-gray-200"
                onClick={() => setSignedIn(false)}
              >
                Sign Out
              </button>
            ) : (
              <button
                className={`px-4 py-2 rounded-md font-medium transition-colors ${
                  canSignIn
                    ? "bg-blue-600 hover:bg-blue-700 text-white"
                    : "bg-gray-300 text-gray-500 cursor-not-allowed"
                }`}
                disabled={!canSignIn}
                onClick={() => canSignIn && setSignedIn(true)}
              >
                Sign In
              </button>
            )}
          </div>
        </div>

        {config.mode === "generated" && (
          <section className="space-y-4">
            <div>
              <label className="block text-sm font-semibold text-gray-700 dark:text-gray-300 mb-2">
                Shared Secret
              </label>
              <input
                type="text"
                className={`w-full px-3 py-2 rounded-lg border font-mono text-sm ${
                  config.secret.readOnly
                    ? "bg-gray-100 dark:bg-gray-700 cursor-not-allowed text-gray-500 dark:text-gray-400"
                    : "bg-white dark:bg-gray-800 text-gray-900 dark:text-gray-100"
                } border-gray-300 dark:border-gray-600 shadow-sm focus:outline-none`}
                value={config.secret.value}
                readOnly={config.secret.readOnly}
                onChange={(event) =>
                  setConfig((current) =>
                    current.updateSharedSecret(event.target.value)
                  )
                }
              />
              {config.secret.readOnly && (
                <p className="mt-1 text-xs text-gray-500 dark:text-gray-400">
                  This secret is provided by the environment and cannot be edited.
                </p>
              )}
            </div>

            <div>
              <label className="block text-sm font-semibold text-gray-700 dark:text-gray-300 mb-2">
                Claims (JSON)
              </label>
              <div className="rounded-lg border border-gray-300 dark:border-gray-600 shadow-sm overflow-hidden">
                <Editor
                  height="12rem"
                  defaultLanguage="json"
                  value={config.claims}
                  onChange={(value) => {
                    const next = value ?? "";
                    setConfig((current) => current.updateClaims(next));
                    try {
                      JSON.parse(next);
                      setClaimsError(null);
                    } catch (error) {
                      setClaimsError((error as Error).message);
                    }
                  }}
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
                <div className="text-red-600 dark:text-red-400 text-sm mt-1">
                  {claimsError}
                </div>
              )}
            </div>
          </section>
        )}

        {config.mode === "static" && (
          <section>
            <label className="block text-sm font-semibold text-gray-700 dark:text-gray-300 mb-2">
              JWT Token
            </label>
            <textarea
              className="w-full px-3 py-2 rounded-lg border bg-white dark:bg-gray-800 text-gray-900 dark:text-gray-100 border-gray-300 dark:border-gray-600 shadow-sm focus:outline-none font-mono text-xs h-32 resize-y"
              value={config.secret.value}
              onChange={(event) =>
                setConfig((current) => current.updateToken(event.target.value))
              }
              placeholder="Paste a JWT token here"
            />
            <p className="mt-1 text-xs text-gray-500 dark:text-gray-400">
              The token is stored only in your browser&apos;s local storage.
            </p>
          </section>
        )}

        <section className="space-y-3">
          <div className="flex items-center justify-between">
            <h3 className="text-sm font-semibold text-gray-700 dark:text-gray-200">
              Custom Headers
            </h3>
            <div className="flex items-center gap-2">
              <input
                type="text"
                className="px-2 py-1 rounded border bg-white dark:bg-gray-800 text-gray-900 dark:text-gray-100 border-gray-300 dark:border-gray-600 shadow-sm focus:outline-none text-xs font-mono w-32"
                placeholder="Header"
                value={newHeaderKey}
                onChange={(event) => setNewHeaderKey(event.target.value)}
              />
              <input
                type="text"
                className="px-2 py-1 rounded border bg-white dark:bg-gray-800 text-gray-900 dark:text-gray-100 border-gray-300 dark:border-gray-600 shadow-sm focus:outline-none text-xs font-mono w-48"
                placeholder="Value"
                value={newHeaderValue}
                onChange={(event) => setNewHeaderValue(event.target.value)}
              />
              <button
                className="text-xs text-blue-600 hover:text-blue-700 dark:text-blue-400 dark:hover:text-blue-300"
                onClick={() => {
                  const key = newHeaderKey.trim();
                  if (!key) {
                    return;
                  }
                  const value = newHeaderValue;
                  updateHeaders(setConfig, (headers) => ({
                    ...headers,
                    [key]: value,
                  }));
                  setNewHeaderKey("");
                  setNewHeaderValue("");
                }}
              >
                Add
              </button>
            </div>
          </div>

          <table className="min-w-full border border-gray-200 dark:border-gray-700 rounded-lg overflow-hidden text-sm">
            <thead className="bg-gray-100 dark:bg-gray-800 text-gray-700 dark:text-gray-300">
              <tr>
                <th className="px-3 py-2 text-left font-medium">Header</th>
                <th className="px-3 py-2 text-left font-medium">Value</th>
                <th className="px-3 py-2 text-right font-medium">Actions</th>
              </tr>
            </thead>
            <tbody>
              {headerEntries.length === 0 && (
                <tr>
                  <td
                    className="px-3 py-3 text-gray-500 dark:text-gray-400 text-center"
                    colSpan={3}
                  >
                    No custom headers configured.
                  </td>
                </tr>
              )}
              {headerEntries.map(([key, value]) => (
                <tr
                  key={key}
                  className="border-t border-gray-200 dark:border-gray-700"
                >
                  <td className="px-3 py-2">
                    <input
                      type="text"
                      className="w-full px-2 py-1 rounded border bg-white dark:bg-gray-800 text-gray-900 dark:text-gray-100 border-gray-300 dark:border-gray-600 shadow-sm focus:outline-none font-mono text-xs"
                      value={key}
                      onChange={(event) => {
                        const nextKey = event.target.value;
                        updateHeaders(setConfig, (headers) => {
                          const newHeaders = { ...headers };
                          const currentValue = newHeaders[key];
                          delete newHeaders[key];
                          if (nextKey.trim()) {
                            newHeaders[nextKey] = currentValue;
                          }
                          return newHeaders;
                        });
                      }}
                    />
                  </td>
                  <td className="px-3 py-2">
                    <input
                      type="text"
                      className="w-full px-2 py-1 rounded border bg-white dark:bg-gray-800 text-gray-900 dark:text-gray-100 border-gray-300 dark:border-gray-600 shadow-sm focus:outline-none font-mono text-xs"
                      value={value}
                      onChange={(event) =>
                        updateHeaders(setConfig, (headers) => ({
                          ...headers,
                          [key]: event.target.value,
                        }))
                      }
                    />
                  </td>
                  <td className="px-3 py-2 text-right">
                    <button
                      className="text-xs text-red-600 hover:text-red-700 dark:text-red-400 dark:hover:text-red-300"
                      onClick={() =>
                        updateHeaders(setConfig, (headers) => {
                          const next = { ...headers };
                          delete next[key];
                          return next;
                        })
                      }
                    >
                      Remove
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </section>

        {roleInfo && (
          <section className="space-y-2">
            <h3 className="text-sm font-semibold text-gray-700 dark:text-gray-200">
              Role Preview
            </h3>
            <div className="bg-blue-50 dark:bg-blue-900/20 rounded-lg p-3 space-y-1 text-sm">
              <div>
                <span className="font-semibold">Available Roles:</span>{" "}
                {roleInfo.availableRoles.join(", ") || "None"}
              </div>
              <div>
                <span className="font-semibold">Default Role:</span>{" "}
                {roleInfo.defaultRole || "None"}
              </div>
              <div>
                <span className="font-semibold">Active Role Override:</span>{" "}
                {config.headers["x-hasura-role"] || "using default role"}
              </div>
            </div>
          </section>
        )}

        {jwtDetails && (
          <section className="space-y-3">
            <div className="flex items-center justify-between">
              <h3 className="text-sm font-semibold text-gray-700 dark:text-gray-200">
                JWT Details
              </h3>
              {config.mode === "generated" && (
                <span className="text-xs text-gray-500 dark:text-gray-400">
                  Preview based on current claims (token issued on sign-in)
                </span>
              )}
            </div>
            {jwtDetails.error ? (
              <div className="text-sm text-red-600 dark:text-red-400">
                {jwtDetails.error}
              </div>
            ) : (
              <>
                {summaryEntries.length > 0 && (
                  <div className="grid gap-2 sm:grid-cols-2">
                    {summaryEntries.map((entry) => (
                      <div
                        key={entry.label}
                        className="rounded-md border border-gray-200 dark:border-gray-700 bg-gray-50 dark:bg-gray-800/60 px-3 py-2"
                      >
                        <div className="text-xs uppercase tracking-wide text-gray-500 dark:text-gray-400">
                          {entry.label}
                        </div>
                        <div className="text-sm text-gray-800 dark:text-gray-200 break-all">
                          {entry.value}
                        </div>
                      </div>
                    ))}
                  </div>
                )}
                <div className="grid gap-4 lg:grid-cols-2">
                  <div>
                    <h4 className="text-xs font-semibold uppercase tracking-wide text-gray-500 dark:text-gray-400 mb-1">
                      Header
                    </h4>
                    <pre className="text-xs bg-gray-900/90 text-gray-100 rounded-md p-3 overflow-auto max-h-48">
                      {stringifyJson(jwtDetails.header) || "—"}
                    </pre>
                  </div>
                  <div>
                    <h4 className="text-xs font-semibold uppercase tracking-wide text-gray-500 dark:text-gray-400 mb-1">
                      Payload
                    </h4>
                    <pre className="text-xs bg-gray-900/90 text-gray-100 rounded-md p-3 overflow-auto max-h-48">
                      {stringifyJson(jwtDetails.payload) || "—"}
                    </pre>
                  </div>
                </div>
                {jwtDetails.signature !== null && (
                  <div>
                    <h4 className="text-xs font-semibold uppercase tracking-wide text-gray-500 dark:text-gray-400 mb-1">
                      Signature
                    </h4>
                    <code className="text-xs break-all bg-gray-100 dark:bg-gray-800 px-2 py-1 rounded">
                      {jwtDetails.signature || "—"}
                    </code>
                  </div>
                )}
              </>
            )}
          </section>
        )}
      </main>
    </div>
  );
}
