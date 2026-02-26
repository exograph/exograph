// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

import { ToolErrorFactory } from '../types/ToolError';
import { Fetcher, FetchResult } from './fetcher';

/**
 * HTTP error with status code information
 */
export class HttpError extends Error {
  constructor(message: string, public status: number) {
    super(message);
    this.name = 'HttpError';
  }

  toString(): string {
    return `HttpError: ${this.message} (status: ${this.status})`;
  }
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
        const httpError = new HttpError(`HTTP error. Status: ${response.status}`, response.status);
        const toolError = ToolErrorFactory.fromHttpError(response.status);

        return {
          type: 'failure' as const,
          error: httpError,
          toolError
        };
      }

      const text = await response.text();
      return { type: 'success', text, status: response.status };
    } catch (error: unknown) {
      // Check for abort error specifically
      if (error instanceof Error && error.name === 'AbortError') {
        return { type: 'aborted' };
      }

      const toolError = ToolErrorFactory.fromUnknownError(error);
      const errorObj = this.toError(error);

      return {
        type: 'failure',
        error: errorObj,
        toolError
      };
    }
  }

  private toError(error: unknown): Error {
    if (error instanceof Error) {
      return error;
    }

    if (typeof error === 'string') {
      return new Error(error);
    }

    if (error && typeof error === 'object' && 'message' in error) {
      return new Error(String(error.message));
    }

    return new Error('Unknown error occurred');
  }

  abort(): void {
    if (this.abortController) {
      this.abortController.abort();
      this.abortController = null;
    }
  }
}