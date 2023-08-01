import { useEffect, useState } from "react";

import { GraphiQLPlugin } from "@graphiql/react";

import { HighlightStyle, syntaxHighlighting } from "@codemirror/language";
import { EditorView } from "@codemirror/view";
import { linter } from "@codemirror/lint";
import { json, jsonParseLinter } from "@codemirror/lang-json";
import CodeMirror from "@uiw/react-codemirror";
import { tags } from "@lezer/highlight";

import * as jose from "jose";

const defaultPayload = `{
  "sub": "1234567890",
  "name": "Jordan Taylor"
}`;

export function authPlugin(
  onTokenChange: (token: string | null) => void
): GraphiQLPlugin {
  return {
    title: "Authentication Manager",
    icon: () => (
      <svg
        fill="none"
        viewBox="0 0 24 24"
        strokeWidth="1.5"
        stroke="currentColor"
      >
        <path
          strokeLinecap="round"
          strokeLinejoin="round"
          d="M15.75 5.25a3 3 0 013 3m3 0a6 6 0 01-7.029 5.912c-.563-.097-1.159.026-1.563.43L10.5 17.25H8.25v2.25H6v2.25H2.25v-2.818c0-.597.237-1.17.659-1.591l6.499-6.499c.404-.404.527-1 .43-1.563A6 6 0 1121.75 8.25z"
        />
      </svg>
    ),
    content: () => <AuthPanel onTokenChange={onTokenChange} />,
  };
}

type AuthPanelProps = {
  onTokenChange: (token: string | null) => void;
};

const jsonExtension = json();
const jsonLinterExtension = linter(jsonParseLinter());

const labelStyle = {
  fontSize: "var(--font-size-h4)",
  fontWeight: "bold",
  marginTop: "0.5rem",
  marginBottom: "0.4rem",
};

let exoTheme = EditorView.theme({
  "&.cm-focused": {
    outline: "none !important",
  },
  ".cm-cursor": {
    borderLeftColor: "hsla(var(--color-neutral),1)",
    borderLeftWidth: "2px",
  },
  ".cm-gutters": {
    backgroundColor: "inherit",
    border: "none",
    color: "hsl(var(--color-neutral), 0.5)",
  },
  ".cm-activeLineGutter": {
    backgroundColor: "inherit",
  },
});

const exoHighlightStyle = HighlightStyle.define([
  { tag: tags.keyword, color: "#fc6" },
  { tag: tags.comment, color: "#f5d", fontStyle: "italic" },
  { tag: tags.string, color: "hsl(var(--color-warning))" },
  { tag: tags.number, color: "hsl(var(--color-success))" },
]);

const codeMirrorStyle = {
  borderRadius: "10px",
  marginBottom: "10px",
  padding: "10px",
  boxShadow: "0px 0px 8px 0px hsla(var(--color-neutral), 0.2)",
};

function AuthPanel({ onTokenChange }: AuthPanelProps) {
  const [payload, setPayload] = useState(defaultPayload);
  const [payloadError, setPayloadError] = useState<string | undefined>(
    undefined
  );
  const [secret, setSecret] = useState("");

  useEffect(() => {
    try {
      JSON.parse(payload);
      setPayloadError(undefined);
    } catch (e) {
      setPayloadError((e as Error).message);
      return;
    }
  }, [payload]);

  const updateAuthorizationToken = () => {
    const updateJwt = async () => {
      try {
        const payloadJson = JSON.parse(payload);
        const token = await createJwtToken(payloadJson, secret);
        setPayloadError(undefined);
        onTokenChange(token);
      } catch (e) {
        setPayloadError((e as Error).message);
        return;
      }
    };

    updateJwt();
  };

  return (
    <div style={{ display: "flex", flexDirection: "column", height: "100%" }}>
      <div className="graphiql-doc-explorer-title">Authentication Manager</div>
      <div style={labelStyle}>Secret</div>
      <CodeMirror
        style={codeMirrorStyle}
        placeholder={"EXO_JWT_SECRET value"}
        value={secret}
        basicSetup={{
          lineNumbers: false,
          foldGutter: false,
          syntaxHighlighting: true,
          highlightActiveLine: false,
        }}
        extensions={[syntaxHighlighting(exoHighlightStyle)]}
        theme={exoTheme}
        onChange={setSecret}
      />
      <div style={labelStyle}>Payload</div>
      <CodeMirror
        style={codeMirrorStyle}
        value={payload}
        minHeight="5rem"
        basicSetup={{
          lineNumbers: false,
          foldGutter: false,
          syntaxHighlighting: true,
          highlightActiveLine: false,
        }}
        extensions={[
          syntaxHighlighting(exoHighlightStyle),
          jsonExtension,
          jsonLinterExtension,
        ]}
        theme={exoTheme}
        onChange={setPayload}
      />
      {
        <div style={{ color: "brown", fontSize: "0.9rem", height: "3rem" }}>
          {payloadError}
        </div>
      }
      <button
        className="graphiql-button"
        style={{ marginTop: "1rem" }}
        onClick={updateAuthorizationToken}
        disabled={payloadError || !secret || !payload ? true : false}
      >
        Update Authorization Token
      </button>
      <div
        style={{ fontSize: "0.9rem", alignSelf: "flex-end", marginTop: "auto" }}
      >
        A JWT token based on the secret and payload will be automatically added
        as the <code>Authorization</code> header.
      </div>
    </div>
  );
}

async function createJwtToken(
  payload: Record<string, unknown>,
  secret: string
): Promise<string | null> {
  if (secret === "") {
    return "";
  }

  const encodedSecret = new TextEncoder().encode(secret);
  const alg = "HS256";

  return await new jose.SignJWT(payload)
    .setProtectedHeader({ alg })
    .setIssuedAt()
    .setExpirationTime("10y") // Set this to a really long expiration time, so that developers don't get confused when the token expires
    .sign(encodedSecret);
}
