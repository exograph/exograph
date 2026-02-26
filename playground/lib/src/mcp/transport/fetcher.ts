import { ToolError } from '../types/ToolError';

export type FetchResult =
  | { type: 'success'; text: string; status: number }
  | { type: 'failure'; error: Error; toolError?: ToolError }
  | { type: 'aborted' };

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