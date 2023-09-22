import { ReactNode } from "react";
import { AuthPlugin } from "../../AuthPlugin";
import { SignInPanel } from "./SignInPanel";
import { UserIcon } from "./UserIcon";
import { AuthConfigProvider } from "./AuthConfigProvider";

export class SecretAuthPlugin implements AuthPlugin {
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
