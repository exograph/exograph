import { ReactNode } from "react";
import { AuthPlugin } from "../../AuthPlugin";
import { SignInPanel } from "./SignInPanel";
import { UserIcon } from "./UserIcon";
import { AuthConfigProvider } from "./AuthConfigProvider";
import { JwtSecret } from "./SecretConfig";

export class SecretAuthPlugin implements AuthPlugin<JwtSecret | undefined> {
  constructor(readonly config: JwtSecret | undefined) {
    this.config = config;
  }

  getAuthConfigProvider(): React.ComponentType<{ children: ReactNode }> {
    return AuthConfigProvider;
  }

  getSignInPanel(): React.ComponentType<{ onDone: () => void }> {
    return SignInPanel;
  }

  getUserIcon(): React.ComponentType<{}> {
    return UserIcon;
  }
}
