export async function registerUser(claytip: any, username: string, email: string): Promise<boolean> {
    // first query
    let result = await claytip.executeQuery(`
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
    await claytip.executeQuery(`
        mutation($id: Int!, $email: String!) {
            user: updateUser(id: $id, data: {
                email: $email
            })
        } 
    `, {
        "id": result.user.id,
        "email": email
    });

    throw new ClaytipError("some user error");

    // as the user's request failed, all changes should be rolled back from the database at this point

    return true;
}