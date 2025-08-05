import { useContext, useEffect, useState } from "react";
import { EditorView } from "@codemirror/view";
import CodeMirror from "@uiw/react-codemirror";
import { linter } from "@codemirror/lint";
import { json, jsonParseLinter } from "@codemirror/lang-json";
import { HighlightStyle, syntaxHighlighting } from "@codemirror/language";

import { tags } from "@lezer/highlight";
import { AuthConfigContext } from "./AuthConfigProvider";
import { SecretAuthContext } from "./SecretAuthProvider";

const jsonExtension = json();
const jsonLinterExtension = linter(jsonParseLinter());

export function SignInPanel(props: { onDone: () => void }) {
  const { config, setConfig } = useContext(AuthConfigContext);
  const { setSignedIn } = useContext(SecretAuthContext);

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

  const secretStyleAdditions = config.secret.readOnly
    ? {
        backgroundColor: "lightgray",
        cursor: "not-allowed",
      }
    : {};

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
        style={{
          ...codeMirrorStyle,
          ...secretStyleAdditions,
        }}
        placeholder={"EXO_JWT_SECRET value"}
        value={jwtSecret}
        editable={!config.secret.readOnly}
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
      <div style={labelStyle}>Claims</div>
      <CodeMirror
        style={codeMirrorStyle}
        value={claims}
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
        onChange={setClaims}
      />
      {
        <div
          style={{
            color: "brown",
            fontSize: "0.9rem",
            height: "2rem",
          }}
        >
          {claimsError}
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
