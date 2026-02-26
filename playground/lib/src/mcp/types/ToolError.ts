// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

export type ToolErrorType = 'http' | 'network' | 'timeout' | 'auth' | 'validation' | 'unknown';

export interface ToolError {
  readonly _toolError: true;
  type: ToolErrorType;
  message: string;
  status?: number;
}

export class ToolErrorFactory {
  private static make(type: ToolErrorType, message: string, status?: number): ToolError {
    return { _toolError: true, type, message, ...(status !== undefined && { status }) };
  }

  static fromHttpError(status: number, message?: string): ToolError {
    const isAuth = status === 401 || status === 403;
    const type: ToolErrorType = isAuth ? 'auth' : status === 400 ? 'validation' : 'http';
    const defaults: Record<number, string> = {
      401: 'Authentication required', 403: 'Access denied',
      400: 'Invalid request', 404: 'Resource not found',
    };
    return this.make(type,
      message || defaults[status] || (status >= 500 ? 'Server error - please try again' : `HTTP error ${status}`),
      status,
    );
  }

  static fromNetworkError(message?: string): ToolError {
    return this.make('network', message || 'Network connection failed');
  }

  static fromTimeoutError(message?: string): ToolError {
    return this.make('timeout', message || 'Request timed out');
  }

  static fromUnknownError(error: unknown, message?: string): ToolError {
    return this.make('unknown',
      message || (error instanceof Error ? error.message : String(error)) || 'Unknown error occurred',
    );
  }

  /**
   * Parse any error-shaped object into a ToolError.
   * Handles MCP errors, HTTP errors, network errors, and raw error objects.
   */
  static parseError(error: unknown): ToolError {
    if (isToolError(error)) return error;

    if (error && typeof error === 'object') {
      const obj = error as Record<string, unknown>;
      const innerRaw = obj.error && typeof obj.error === 'object' ? obj.error : obj;
      const inner = innerRaw as Record<string, unknown>;
      const status = inner.status || inner.statusCode;
      if (status && typeof status === 'number') return this.fromHttpError(status, inner.message as string | undefined);
      if (inner.code === 'NETWORK_ERROR' || inner.type === 'network') return this.fromNetworkError(inner.message as string | undefined);
      if (inner.code === 'TIMEOUT' || inner.type === 'timeout') return this.fromTimeoutError(inner.message as string | undefined);
      return this.fromUnknownError(error, (inner.message || inner.msg) as string | undefined);
    }

    return this.fromUnknownError(error);
  }
}

export function isToolError(obj: unknown): obj is ToolError {
  return obj != null && typeof obj === 'object' && (obj as Record<string, unknown>)._toolError === true;
}

export function isErrorStatus(status: { type: string }): boolean {
  return status.type === "incomplete" && 'reason' in status && (status as Record<string, unknown>).reason === "error";
}

export function extractToolError(result: unknown): ToolError | unknown {
  if (isToolError(result)) {
    return result;
  }
  if (result != null && typeof result === 'object') {
    const obj = result as Record<string, unknown>;
    return obj._toolError || obj.error || result;
  }
  return result;
}

export function isAuthToolError(result: unknown, status: { type: string }): boolean {
  if (result != null && typeof result === 'object') {
    const obj = result as Record<string, unknown>;
    const errorObj = obj.error != null && typeof obj.error === 'object' ? obj.error as Record<string, unknown> : null;
    const resultStatus = obj.status || errorObj?.status;
    if (resultStatus === 401 || resultStatus === 403) return true;

    if (isErrorStatus(status)) {
      const error = obj.error || obj._toolError;
      if (isToolError(error) && (error.type === "auth" || error.status === 401 || error.status === 403)) {
        return true;
      }
    }
  }

  return false;
}
