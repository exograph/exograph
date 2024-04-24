import { ReactNode } from "react";
import { AuthPlugin } from "../../AuthPlugin";
import { SignInPanel } from "./SignInPanel";
import { AuthConfigProvider } from "./AuthConfigProvider";
import { UserIcon } from "./UserIcon";

export class Auth0AuthPlugin implements AuthPlugin<undefined> {
  config = undefined;

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
