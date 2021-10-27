//import * as bcrypt from 'https://deno.land/x/crypt@v0.1.0/bcrypt.ts';
import { createJwt, queryUserInfo, secret } from "./login-utils.ts";

// TODO: Make bcrypt work
export async function loginNormal(email: string, password: string, claytip: any): Promise<string> {
    const res = claytip.executeQuery(`
        query ($email: String!) {
            users(where: { email: { eq: $email }}) {
                password
            }
        } 
    `, {
        email: email,
    });

    const user = res.users[0];

    //if (await bcrypt.compare(password, user.password)) {
    if (password == user.password) {
        const userInfo = await queryUserInfo(email, claytip);
        return await createJwt(userInfo, secret);
    } else {
        throw new Error(`Incorrect password`);
    }
}

//import { hash } from "https://deno.land/x/bcrypt@v0.2.4/mod.ts";

export async function signupNormal(
    email: string,
    password: string,
    name: string,
    claytip: any
): Promise<string> {
    //let hashed = hash(password);
    let hashed = password;

    let res = claytip.executeQuery(
        `mutation(
          $email: String!, 
          $password: String!,
          $role: String!,
          $name: String!
        ) {
            createUser(data: {
              email: $email, 
              password: $password,
              role: $role,
              name: $name
            }) {
              id
            }
        }`,
        {
            email: email,
            password: hashed,
            role: "USER",
            name: name
        }
    );

    // TODO: Send a verification email and implement a veryfyNormal function to respond to it
    return "Ok";
}