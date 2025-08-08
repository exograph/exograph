// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

import {
  createContext,
  useContext,
  useEffect,
  useState,
  ReactNode,
} from "react";
import { AuthContext } from "../../auth/AuthContext";
import { JWTSource } from "../../auth/types";
import {
  MCPConnectionManager,
  MCPConnectionState,
} from "../api/client/MCPConnectionManager";

const MCPClientContext = createContext<MCPConnectionState | null>(null);

interface MCPClientProviderProps {
  children: ReactNode;
  endpoint: string;
  auth: JWTSource;
}

export function MCPClientProvider({
  children,
  endpoint,
  auth,
}: MCPClientProviderProps) {
  const { getTokenFn } = useContext(AuthContext);
  const [connectionManager] = useState(() => new MCPConnectionManager());
  const [state, setState] = useState<MCPConnectionState>({
    type: "disconnected",
  });

  // Set up listener for state changes
  useEffect(() => {
    const cleanup = connectionManager.addStateListener(setState);
    setState(connectionManager.getState());
    return cleanup;
  }, [connectionManager]);

  // Set up token provider and reconnect if needed
  useEffect(() => {
    if (getTokenFn) {
      connectionManager.setTokenProvider(getTokenFn);
      // Reconnect if we're already connected to use the new token provider
      if (endpoint && auth && connectionManager.isConnected()) {
        connectionManager.connect(endpoint, auth);
      }
    }
  }, [connectionManager, getTokenFn, endpoint, auth]);

  // Connect when endpoint or auth changes
  useEffect(() => {
    if (endpoint && auth) {
      connectionManager.connect(endpoint, auth);
    } else {
      connectionManager.disconnect();
    }
  }, [connectionManager, endpoint, auth]);

  // Cleanup on unmount
  useEffect(() => {
    return () => {
      connectionManager.disconnect();
    };
  }, [connectionManager]);

  return (
    <MCPClientContext.Provider value={state}>
      {children}
    </MCPClientContext.Provider>
  );
}

export function useMCPClient() {
  const state = useContext(MCPClientContext);
  if (!state) {
    throw new Error("useMCPClient must be used within a MCPClientProvider");
  }
  return state;
}
