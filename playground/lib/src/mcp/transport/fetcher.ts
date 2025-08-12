/**
 * Result of a fetch operation as a discriminated union
 */
export type FetchResult =
  | { type: 'success'; text: string; status: number }
  | { type: 'failure'; error: Error; status?: number }
  | { type: 'aborted' };

/**
 * HTTP error with status code information
 */
export class HttpError extends Error {
  constructor(message: string, public status: number) {
    super(message);
    this.name = 'HttpError';
  }
}

/**
 * Interface for abstracting the fetch operation in ExographTransport
 */
export interface Fetcher {
  /**
   * Perform a fetch operation with the given request
   * This method should never throw - all errors should be returned as FetchResult
   * @param request The request body as a string
   * @returns A FetchResult indicating success, failure, or abort
   */
  fetch(request: string): Promise<FetchResult>;

  /**
   * Cancel any ongoing fetch operations
   */
  abort(): void;
}

export class HttpFetcher implements Fetcher {
  private endpoint: string;
  private headers: () => Promise<Record<string, string>>;
  private abortController: AbortController | null = null;

  constructor(endpoint: string, headers: () => Promise<Record<string, string>>) {
    this.endpoint = endpoint;
    this.headers = headers;
    this.abortController = new AbortController();
  }

  async fetch(request: string): Promise<FetchResult> {
    if (!this.abortController) {
      return { type: 'aborted' };
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
        return {
          type: 'failure',
          error: new HttpError(`HTTP error! status: ${response.status}`, response.status),
          status: response.status
        };
      }

      const text = await response.text();
      return { type: 'success', text, status: response.status };
    } catch (error) {
      if (error instanceof Error && error.name === 'AbortError') {
        return { type: 'aborted' };
      }

      // For other errors (network errors, etc.), return as failure
      return {
        type: 'failure',
        error: error instanceof Error ? error : new Error(String(error))
      };
    }
  }

  abort(): void {
    if (this.abortController) {
      this.abortController.abort();
      this.abortController = null;
    }
  }
}