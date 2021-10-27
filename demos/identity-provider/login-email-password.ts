//import * as bcrypt from 'https://deno.land/x/crypt@v0.1.0/bcrypt.ts';
import { createJwt, queryUserInfo, secret } from "./login-utils.ts";

export async function login(email: string, password: string, claytip: any): Promise<string> {
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