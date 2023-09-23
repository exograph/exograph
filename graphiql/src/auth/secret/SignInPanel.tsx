import { useContext, useEffect, useState } from "react";
import { EditorView } from "@codemirror/view";
import CodeMirror from "@uiw/react-codemirror";
import { linter } from "@codemirror/lint";
import { json, jsonParseLinter } from "@codemirror/lang-json";
import { HighlightStyle, syntaxHighlighting } from "@codemirror/language";

import { tags } from "@lezer/highlight";
import { SecretConfig } from "./SecretConfig";
import { AuthConfigContext } from "./AuthConfigProvider";
import { SecretAuthContext } from "./SecretAuthProvider";

const jsonExtension = json();
const jsonLinterExtension = linter(jsonParseLinter());

export function SignInPanel(props: { onDone: () => void }) {
  const { config, setConfig } = useContext(AuthConfigContext);
  const { setSignedIn } = useContext(SecretAuthContext);

  const [jwtSecret, setJwtSecret] = useState(config.secret);
  const [payload, setPayload] = useState(config.payload || "");
  const [payloadError, setPayloadError] = useState<string | undefined>(
    undefined
  );

  useEffect(() => {
    try {
      JSON.parse(payload);
      setPayloadError(undefined);
    } catch (e) {
      setPayloadError((e as Error).message);
      return;
    }
  }, [payload]);

  const enableSignIn = !payloadError && jwtSecret && payload ? true : false;

  function onSignIn() {
    setConfig(new SecretConfig(jwtSecret, payload));
    setSignedIn(true);
    props.onDone();
  }

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
            background: enableSignIn
              ? "hsla(var(--color-tertiary), 1)"
              : "hsla(var(--color-tertiary), 0.4)",
            color: "white",
          }}
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
