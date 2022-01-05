import { verifyGoogle } from "./login-google.ts";
import { createJwt, queryUserInfo, secret } from "./login-utils.ts";

export interface LoginResult {
  email: string;
  givenName: string;
  familyName: string;
  name: string;
  profilePicture: string;
}

// Unlike "normal" login (whose signup requires an email verification step), with social login,
// the user is created immediately (thus we can issue the JWT token on signup immediately).
// So the real difference between login and signup is that the latter adds the new user to the database.
export async function loginSocial(code: string, provider: string, claytip: any): Promise<string> {
  return await helper(code, provider, claytip, undefined);
}

export async function signupSocial(code: string, provider: string, claytip: any): Promise<string> {
  return await helper(code, provider, claytip, signup);
}

type OnSignupFunction = (email: string, name: string, claytip: any) => Promise<string> | undefined;

async function helper(code: string, provider: string, claytip: any, onSignup: OnSignupFunction): Promise<string> {
  if (provider === 'google') {
    const googleUser: LoginResult = await verifyGoogle(code);

    if (onSignup) {
      await onSignup(googleUser.email, `${googleUser.givenName} ${googleUser.familyName}`, claytip);
    }
    const payload = await queryUserInfo(googleUser.email, claytip);
    const token = await createJwt(payload, secret)

    return token
  } else {
    throw new Error(`Unknown provider ${provider}`);
  }
}

async function signup(
  email: string,
  name: string,
  claytip: any
) {
  let res = await claytip.executeQuery(
    `mutation(
        $email: String!, 
        $role: String!,
        $name: String!
      ) {
          createUser(data: {
            email: $email, 
            role: $role,
            name: $name
          }) {
            id
          }
      }`,
    {
      email: email,
      role: "USER",
      name: name
    }
  );

  return res.createUser.id;
}