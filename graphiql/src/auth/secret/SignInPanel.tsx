import { useContext, useEffect, useState } from "react";
import * as jose from "jose";
import { AuthContext } from "../../AuthContext";
import { EditorView } from "@codemirror/view";
import CodeMirror from "@uiw/react-codemirror";
import { linter } from "@codemirror/lint";
import { json, jsonParseLinter } from "@codemirror/lang-json";
import { HighlightStyle, syntaxHighlighting } from "@codemirror/language";

import { tags } from "@lezer/highlight";

const defaultPayload = `{
  "sub": "1234567890",
  "name": "Jordan Taylor"
}`;

type PanelState = {
  jwtSecret: string | undefined;
  payload: string;
};

const paneState: PanelState = {
  jwtSecret: undefined,
  payload: defaultPayload,
};

const jsonExtension = json();
const jsonLinterExtension = linter(jsonParseLinter());

export function SignInPanel(props: { onDone: () => void }) {
  const [jwtSecret, setJwtSecret] = useState<string | undefined>(
    paneState.jwtSecret
  );
  const [payload, setPayload] = useState<string>(paneState.payload);

  const [payloadError, setPayloadError] = useState<string | undefined>(
    undefined
  );

  const { setTokenFn, isSignedIn, setIsSignedIn } = useContext(AuthContext);

  useEffect(() => {
    setTokenFn &&
      jwtSecret &&
      setTokenFn(() =>
        Promise.resolve(createJwtToken(JSON.parse(payload), jwtSecret))
      );
  }, [payload, jwtSecret, setTokenFn]);

  useEffect(() => {
    try {
      JSON.parse(payload);
      setPayloadError(undefined);
    } catch (e) {
      setPayloadError((e as Error).message);
      return;
    }
  }, [payload]);

  useEffect(() => {
    paneState.jwtSecret = jwtSecret;
    paneState.payload = payload;
  }, [jwtSecret, payload]);

  const enableSignIn = !payloadError && jwtSecret && payload ? true : false;

  return (
    <div
      style={{
        display: "flex",
        flexDirection: "column",
        width: "100%",
      }}
    >
      <div style={labelStyle}>Secret</div>
      <CodeMirror
        style={codeMirrorStyle}
        placeholder={"EXO_JWT_SECRET value"}
        value={jwtSecret}
        basicSetup={{
          lineNumbers: false,
          foldGutter: false,
          syntaxHighlighting: true,
          highlightActiveLine: false,
        }}
        extensions={[syntaxHighlighting(exoHighlightStyle)]}
        theme={exoTheme}
        onChange={setJwtSecret}
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
        <div
          style={{
            color: "brown",
            fontSize: "0.9rem",
            height: "2rem",
          }}
        >
          {payloadError}
        </div>
      }
      <div style={{ display: "flex", gap: "1rem", justifyContent: "end" }}>
        <button
          className="graphiql-button"
          style={{
            background: enableSignIn ? "hsla(var(--color-tertiary), 1)" : "hsla(var(--color-tertiary), 0.4)",
            color: "white",
          }}
          onClick={() => {
            setIsSignedIn && setIsSignedIn(true);
            props.onDone();
          }}
          disabled={!enableSignIn}
        >
          Sign In
        </button>
        <button
          className="graphiql-button"
          style={{
            background: isSignedIn ? "hsla(var(--color-secondary), 1)" : "hsla(var(--color-secondary), 0.4)",
            color: "white",
          }}
          disabled={!isSignedIn}
          onClick={() => {
            setTokenFn && setTokenFn(undefined);
            setIsSignedIn && setIsSignedIn(false);
            props.onDone();
          }}
        >
          Sign Out
        </button>
      </div>
    </div>
  );
}

async function createJwtToken(
  payload: Record<string, unknown>,
  secret: string
): Promise<string | null> {
  if (secret === "") {
    return null;
  }

  const encodedSecret = new TextEncoder().encode(secret);
  const alg = "HS256";

  return await new jose.SignJWT(payload)
    .setProtectedHeader({ alg })
    .setIssuedAt()
    .setExpirationTime("10m")
    .sign(encodedSecret);
}

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

const labelStyle = {
  fontSize: "var(--font-size-h4)",
  fontWeight: "bold",
  marginTop: "0.5rem",
  marginBottom: "0.4rem",
};

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
