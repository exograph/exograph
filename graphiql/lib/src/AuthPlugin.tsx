import React, { ReactNode } from "react";

export interface AuthPlugin<C> {
  config: C;

  getAuthConfigProvider(): React.ComponentType<{ children: ReactNode }>;
  getSignInPanel(): React.ComponentType<{ onDone: () => void }>;
  getUserIcon(): React.ComponentType<{}>;
}
