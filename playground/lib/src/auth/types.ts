// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

import { JwtSecret } from "./secret";

export interface JWTSource {
  jwtSourceHeader?: string;
  jwtSourceCookie?: string;
}

export interface JWTAuthentication extends JWTSource {
  oidcUrl?: string;
  jwtSecret?: JwtSecret;
}

/**
 * Apply JWT authentication by either setting a cookie or returning headers.
 * Returns the headers to add to the request.
 */
export function applyJWTAuth(
  auth: JWTSource,
  authToken: string
): Record<string, string> {
  const { jwtSourceCookie, jwtSourceHeader } = auth;

  if (jwtSourceCookie) {
    document.cookie = `${jwtSourceCookie}=${authToken}; Secure; SameSite=Strict; Path=/`;
    return {};
  } else {
    const headerName = jwtSourceHeader || 'Authorization';
    return { [headerName]: `Bearer ${authToken}` };
  }
}
