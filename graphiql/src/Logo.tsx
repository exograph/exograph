import { useTheme } from "./theme";

function ExographLogo() {
  const theme = useTheme();

  // Currently, switching mode in GraphiQL doesn't update the logo, but this will get fixed
  // when https://github.com/graphql/graphiql/pull/2971 is merged.
  const logo = theme === "dark" ? "logo-dark.svg" : "logo-light.svg";

  return (
    <a
      href="https://exograph.dev"
      target="_blank"
      rel="noreferrer"
      style={{ lineHeight: 0 }} // Otherwise, the logo is not vertically centered
    >
      <img src={logo} className="logo" alt="Exograph" />
    </a>
  );
}

export function Logo() {
  return (
    <div style={{ display: "flex", alignItems: "center", gap: "1rem" }}>
      <ExographLogo />
      {/* <UserProfile /> */}
    </div>
  );
}
