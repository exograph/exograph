//import { hash } from "https://deno.land/x/bcrypt@v0.2.4/mod.ts";

export async function signup(
  email: string,
  password: string,
  name: string,
  claytip: any
) {
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

  return res.createUser.id;
}