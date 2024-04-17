import React, { ReactNode } from "react";

export interface AuthPlugin {
  getAuthConfigProvider(): React.ComponentType<{ children: ReactNode }>;
  getSignInPanel(): React.ComponentType<{ onDone: () => void }>;
  getUserIcon(): React.ComponentType<{}>;
}
