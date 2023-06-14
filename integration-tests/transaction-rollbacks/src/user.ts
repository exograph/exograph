import type { ExographError } from "../generated/exograph.d.ts";

export async function registerUser(exograph: any, username: string, email: string): Promise<boolean> {
    // first query
    const result = await exograph.executeQuery(`
        mutation($username: String!) {
            user: createUser(data: {
                username: $username
            }) {
                id
            }
        } 
    `, {
        "username": username
    });

    // second query
    await exograph.executeQuery(`
        mutation($id: Int!, $email: String!) {
            user: updateUser(id: $id, data: {
                email: $email
            })
        } 
    `, {
        "id": result.user.id,
        "email": email
    });

    throw new ExographError("some user error");

    // as the user's request failed, all changes should be rolled back from the database at this point

}