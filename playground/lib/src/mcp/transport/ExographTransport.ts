import { Transport, TransportSendOptions } from '@modelcontextprotocol/sdk/shared/transport.js';
import { JSONRPCMessage, MessageExtraInfo } from '@modelcontextprotocol/sdk/types.js';
import { Fetcher } from './fetcher';

/**
 * A simplified transport layer to meet the needs of Exograph MCP Server.
 * 
 * It is aware that Exograph MCP server is stateless, hence doesn't send the `GET` request to check for it.
 * Also abstracts over the Fetch API so that we can make it work with non-http protocols.
 */
export class ExographTransport implements Transport {
  private fetcher: Fetcher;

  constructor(fetcher: Fetcher) {
    this.fetcher = fetcher;
  }

  async start(): Promise<void> {
    // Transport is ready - no initialization needed
  }

  async send(message: JSONRPCMessage, _options?: TransportSendOptions): Promise<void> {
    const requestBody = JSON.stringify(message);

    try {
      const text = await this.fetcher.fetch(requestBody);

      if (text.trim()) {
        try {
          const responseData = JSON.parse(text) as JSONRPCMessage;
          this.handleMessage(responseData);
        } catch (error) {
          console.error('Failed to parse JSON response:', text);
          throw new Error(`Invalid JSON response: ${text.substring(0, 100)}`);
        }
      } else {
        // Empty response - check if this was a request that needs a response
        if ('id' in message && message.id !== undefined) {
          console.warn('Empty response for request:', message);
        }
      }
    } catch (error) {
      if (error instanceof Error && error.message === 'Request was cancelled') {
        // Request was cancelled, don't treat as error
        return;
      }
      this.handleError(error as Error);
      throw error;
    }
  }

  async close(): Promise<void> {
    this.fetcher.abort();

    if (this.onclose) {
      this.onclose();
    }
  }

  private handleMessage(message: JSONRPCMessage): void {
    if (this.onmessage) {
      const extra: MessageExtraInfo = {};
      this.onmessage(message, extra);
    }
  }

  private handleError(error: Error): void {
    if (this.onerror) {
      this.onerror(error);
    }
  }

  onclose?: (() => void) | undefined;
  onerror?: ((error: Error) => void) | undefined;
  onmessage?: ((message: JSONRPCMessage, extra?: MessageExtraInfo) => void) | undefined;
  sessionId?: string | undefined;
  setProtocolVersion?: ((version: string) => void) | undefined;
}
