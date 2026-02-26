import { Transport, TransportSendOptions } from '@modelcontextprotocol/sdk/shared/transport.js';
import { JSONRPCMessage, MessageExtraInfo, isJSONRPCRequest } from '@modelcontextprotocol/sdk/types.js';
import { Fetcher } from './fetcher';

/**
 * A simplified transport layer to meet the needs of Exograph MCP Server.
 * 
 * It is aware that Exograph MCP server is stateless, hence doesn't send the `GET` request to check for it.
 * Also abstracts over the Fetch API so that we can make it work with non-http protocols.
 */
export class ExographTransport implements Transport {
  private fetcher: Fetcher;

  onclose?: (() => void) | undefined;
  onerror?: ((error: Error) => void) | undefined;
  onmessage?: ((message: JSONRPCMessage, extra?: MessageExtraInfo) => void) | undefined;
  sessionId?: string | undefined;
  setProtocolVersion?: ((version: string) => void) | undefined;

  constructor(fetcher: Fetcher) {
    this.fetcher = fetcher;
  }

  async start(): Promise<void> {
  }

  async send(message: JSONRPCMessage, _options?: TransportSendOptions): Promise<void> {
    const requestBody: string = JSON.stringify(message);

    const result = await this.fetcher.fetch(requestBody);

    // Check if this is a request that expects a response
    const isRequest = isJSONRPCRequest(message);

    switch (result.type) {
      case 'success':
        if (result.text.trim()) {
          try {
            const responseData = JSON.parse(result.text) as JSONRPCMessage;

            // Check for JSON-RPC error in response
            if ('error' in responseData && responseData.error) {
              let errorMessage = 'Unknown error';
              if (typeof responseData.error === 'object' && responseData.error !== null) {
                if ('message' in responseData.error && typeof responseData.error.message === 'string') {
                  errorMessage = responseData.error.message;
                } else {
                  errorMessage = JSON.stringify(responseData.error);
                }
              } else if (typeof responseData.error === 'string') {
                errorMessage = responseData.error;
              }

              const error = new Error(errorMessage);
              this.onerror?.(error);
              if (isRequest) {
                throw error;
              }
              return;
            }

            this.onmessage?.(responseData);
          } catch (error) {
            const parseError = new Error(
              `Failed to parse JSON response: ${result.text.substring(0, 100)}${result.text.length > 100 ? '...' : ''
              }`
            );
            this.onerror?.(parseError);
            if (isRequest) {
              throw parseError;
            }
          }
        } else {
          // Empty response - error if this was a request expecting a response
          if (isRequest) {
            const requestId = message.id;
            const emptyResponseError = new Error(
              `Empty response received for request with id: ${requestId}`
            );
            this.onerror?.(emptyResponseError);
            throw emptyResponseError;
          }
        }
        break;

      case 'failure':
        // Report error via onerror callback
        this.onerror?.(result.error);
        // Throw for requests expecting responses
        if (isRequest) {
          // Preserve ToolError information by attaching it to the thrown error
          if (result.toolError) {
            (result.error as any).toolError = result.toolError;
          }
          throw result.error;
        }
        break;

      case 'aborted':
        // Silently handle abort - this is expected during shutdown
        break;
    }
  }

  async close(): Promise<void> {
    this.fetcher.abort();

    if (this.onclose) {
      this.onclose();
    }
  }
}
