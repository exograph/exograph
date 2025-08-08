/**
 * Interface for abstracting the fetch operation in ExographTransport
 */
export interface Fetcher {
  /**
   * Perform a fetch operation with the given request
   * @param request The request body as a string
   * @returns The response text
   */
  fetch(request: string): Promise<string>;

  /**
   * Cancel any ongoing fetch operations
   */
  abort(): void;
}

/**
 * HTTP-based implementation of the Fetcher interface
 */
export class HttpFetcher implements Fetcher {
  private endpoint: string;
  private headers: () => Promise<Record<string, string>>;
  private abortController: AbortController | null = null;

  constructor(endpoint: string, headers: () => Promise<Record<string, string>>) {
    this.endpoint = endpoint;
    this.headers = headers;
    this.abortController = new AbortController();
  }

  async fetch(request: string): Promise<string> {
    if (!this.abortController) {
      throw new Error('Fetcher has been aborted');
    }

    try {
      let headers = await this.headers();
      const response = await globalThis.fetch(this.endpoint, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          ...headers,
        },
        body: request,
        signal: this.abortController.signal
      });

      if (!response.ok) {
        throw new Error(`HTTP error! status: ${response.status}`);
      }

      return response.text();
    } catch (error) {
      if (error instanceof Error && error.name === 'AbortError') {
        // Convert HTTP-specific AbortError to a generic error
        throw new Error('Request was cancelled');
      }
      throw error;
    }
  }

  abort(): void {
    if (this.abortController) {
      this.abortController.abort();
      this.abortController = null;
    }
  }
}