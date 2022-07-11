const existingRsvpQuery = `
  query rsvps($email: String) {
    rsvps(where: {email: {eq: $email}}) {
      id
    }
  }
`;

const updateRsvpMutation = `
  mutation updateRsvp($id: Int!, $rsvp: RsvpUpdateInput!) {
    updateRsvp(id: $id, data: $rsvp) {
      id
    }
  }
`;

const adminContext = {
  AuthContext: {
    role: 'admin',
  }
}

export async function processRsvp(operation: Operation, claytip: ClaytipPriv) {
  const query = operation.query();
  const email = query.arguments.data.email;
  const existing = await claytip.executeQueryPriv(existingRsvpQuery, {
    email,
  }, adminContext);

  if (existing.rsvps.length > 0) {
    const existingRsvp = existing.rsvps[0];
    const count = query.arguments.data.count;

    return (await claytip.executeQueryPriv(updateRsvpMutation, {
      id: existingRsvp.id,
      rsvp: {
        email,
        count,
      }
    }, adminContext)).updateRsvp;

  };

  return await operation.proceed();
}