import { RSA, encode } from "https://deno.land/x/god_crypto@v1.4.8/mod.ts";
import { decode } from "https://deno.land/x/djwt@v2.4/mod.ts";

import { createJwt, queryUserInfo, secret } from "./login-utils.ts";


interface LoginResult {
  email: string;
  givenName: string;
  familyName: string;
  name: string;
  profilePicture: string;
}

interface GoogleJwtPayload {
  email: string;
  "email_verified": boolean;
  name: string;
  "given_name": string;
  "family_name": string;
  picture: string;
  sub: string;
  exp: number;
}

interface GoogleJwtHeader {
  kid: string;
}

interface JWK {
  kid: string;
}


// Take the code from the client-side authentication and issue a JWT token
export async function loginGoogle(code: string, claytip: any): Promise<string> {
  const googleUser = await verifyGoogle(code);
  const payload = await queryUserInfo(googleUser.email, claytip);
  const token = await createJwt(payload, secret)

  return token
}

export async function verifyGoogle(jwt: string): Promise<LoginResult> {
  const certs = await fetch("https://www.googleapis.com/oauth2/v3/certs"); // TODO: cache using cache-control
  const jwks = await certs.json();

  const [header, payload, _signature] = decode(jwt) as [GoogleJwtHeader, GoogleJwtPayload, unknown];

  const exp = payload.exp;
  const now = Math.floor(Date.now() / 1000);

  if (exp <= now) {
    throw new Error("Token expired");
  }

  const result: LoginResult = {
    email: payload.email,
    givenName: payload.given_name,
    familyName: payload.family_name,
    name: payload.name,
    profilePicture: payload.picture
  }

  const pubjwk = jwks.keys.find((key: JWK) => key.kid === header.kid);

  if (pubjwk) {
    const publicKey = RSA.parseKey(pubjwk)
    const rsa = new RSA(publicKey)

    const [headerb64, payloadb64, signatureb64] = jwt.split(".")

    const verified = await rsa.verify(
      encode.base64url(signatureb64),
      headerb64 + "." + payloadb64
    );

    if (!verified) {
      throw new Error("Invalid signature");
    } else {
      return result;
    }
  } else {
    throw new Error(`key with kid ${header.kid} not found`);
  }
}

