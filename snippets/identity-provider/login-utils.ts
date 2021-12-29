import { create, getNumericDate } from "https://deno.land/x/djwt@v2.4/mod.ts";

export const secret = Deno.env.get("CLAY_JWT_SECRET");

export interface JWTPayload {
  sub: string;
  role: string;
  email: string;
  name: string;
}

export async function createJwt(payload: JWTPayload, secret: string): Promise<string> {
  const encoder = new TextEncoder()
  const keyBuf = encoder.encode(secret);
  const key = await crypto.subtle.importKey(
    "raw",
    keyBuf,
    { name: "HMAC", hash: "SHA-256" },
    true,
    ["sign", "verify"],
  );

  return await create({ alg: "HS256", typ: "JWT" }, { exp: getNumericDate(60 * 60), ...payload }, key)
}

export async function queryUserInfo(email: string, claytip: any): Promise<JWTPayload> {
  const res = await claytip.executeQuery(`
        query ($email: String!) {
            users(where: { email: { eq: $email }}) {
                id
                role
                name
                password
            }
        } 
    `, {
    email: email,
  });

  if (res.users.length === 0) {
    throw new Error("User not found");
  }

  let user = res.users[0];

  const payload = {
    sub: user.id,
    role: user.role,
    name: user.name,
    email: email,
  };

  return payload;
}