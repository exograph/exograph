// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

import { createMCPClient, type MCPClient } from '@ai-sdk/mcp';
import { ExographTransport, HttpFetcher } from '../../transport';
import { JWTSource } from '../../../auth/types';

export type MCPConnectionState =
  | { type: 'disconnected' }
  | { type: 'connecting' }
  | { type: 'connected'; client: MCPClient; tools: Record<string, any> }
  | { type: 'failed'; error: string; retry: () => void };

export class MCPConnectionManager {
  private state: MCPConnectionState = { type: 'disconnected' };
  private listeners: Set<(state: MCPConnectionState) => void> = new Set();
  private currentEndpoint: string | null = null;
  private currentAuth: JWTSource | null = null;
  private getTokenFn: (() => Promise<string | null>) | null = null;

  addStateListener(listener: (state: MCPConnectionState) => void): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  private notifyListeners(): void {
    this.listeners.forEach((listener) => listener(this.state));
  }

  getState(): MCPConnectionState {
    return this.state;
  }

  private setState(state: MCPConnectionState): void {
    this.state = state;
    this.notifyListeners();
  }

  setTokenProvider(getTokenFn: () => Promise<string | null>): void {
    this.getTokenFn = getTokenFn;
  }

  private async computeHeaders(): Promise<Record<string, string>> {
    const headers: Record<string, string> = {};

    if (this.getTokenFn && this.currentAuth) {
      const { jwtSourceCookie, jwtSourceHeader } = this.currentAuth;
      const authToken = await this.getTokenFn();

      if (authToken && typeof authToken === 'string' && authToken.trim()) {
        if (jwtSourceCookie) {
          // Set secure cookie with proper flags
          const cookieValue = `${jwtSourceCookie}=${authToken}; Secure; SameSite=Strict; Path=/`;
          document.cookie = cookieValue;
        }

        if (jwtSourceHeader) {
          const authHeaderName = jwtSourceHeader || 'Authorization';
          headers[authHeaderName] = `Bearer ${authToken}`;
        }
      }
    }

    return headers;
  }

  async connect(endpoint: string, auth: JWTSource): Promise<void> {
    // Don't reconnect if already connecting to the same endpoint
    if (
      this.state.type === 'connecting' &&
      this.currentEndpoint === endpoint
    ) {
      return;
    }

    await this.disconnect();

    this.currentEndpoint = endpoint;
    this.currentAuth = auth;
    this.setState({ type: 'connecting' });

    try {
      const fetcher = new HttpFetcher(endpoint, () => this.computeHeaders());
      const transport = new ExographTransport(fetcher);

      const client = await createMCPClient({
        transport,
      });

      const tools = await client.tools();

      this.setState({
        type: 'connected',
        client,
        tools,
      });
    } catch (error) {
      console.error('Failed to connect to MCP server:', error);

      // Sanitize error message to prevent information disclosure
      const sanitizedError =
        error instanceof Error
          ? error.message.includes('Invalid endpoint')
            ? error.message
            : 'Connection failed'
          : 'Connection failed';

      this.setState({
        type: 'failed',
        error: sanitizedError,
        retry: () => this.retry(),
      });
    }
  }

  async retry(): Promise<void> {
    if (this.currentEndpoint && this.currentAuth) {
      await this.connect(this.currentEndpoint, this.currentAuth);
    } else {
      this.setState({ type: 'disconnected' });
    }
  }

  async disconnect(): Promise<void> {
    if (this.state.type === 'connected') {
      try {
        await this.state.client.close();
      } catch (error) {
        console.error('Error closing MCP client:', error);
      }
    }

    this.currentEndpoint = null;
    this.currentAuth = null;
    this.setState({ type: 'disconnected' });
  }

  isConnected(): boolean {
    return this.state.type === 'connected';
  }

  getTools(): Record<string, any> | null {
    return this.state.type === 'connected' ? this.state.tools : null;
  }
}