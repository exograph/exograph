const existingRsvpQuery = `
  query rsvps($email: String) {
    rsvps(where: {email: {eq: $email}}) {
      id
      email
      count
    }
  }
`;

const createRsvpMutation = `
  mutation ($email: String, $count: Int!) {
    createRsvp(data: {email: $email, count: $count}) {
      id
      email
      count
    }
  }
`;

const updateRsvpMutation = `
  mutation updateRsvp($id: Int!, $rsvp: RsvpUpdateInput!) {
    updateRsvp(id: $id, data: $rsvp) {
      id
      email
      count
    }
  }
`;

const adminContext = {
  AuthContext: {
    role: 'admin',
  }
}

interface RsvpConfirmation {
  email: string
  count: number
}

// Perform an upsert operation. In real world app, this would also send a confirmation email etc.
export async function processRsvp(email: string, count: number, claytip: ClaytipPriv): Promise<RsvpConfirmation> {
  const existing = await claytip.executeQueryPriv(existingRsvpQuery, {
    email,
  }, adminContext);

  if (existing.rsvps.length > 0) {
    const existingRsvp = existing.rsvps[0];

    if (existingRsvp.count !== count) {
      return (await claytip.executeQueryPriv(updateRsvpMutation, {
        id: existingRsvp.id,
        rsvp: {
          email,
          count,
        }
      }, adminContext)).updateRsvp;
    } else {
      return existingRsvp;
    }
  };
  console.log(`No existing RSVP for ${email}`);

  return (await claytip.executeQueryPriv(createRsvpMutation, {
    email,
    count
  }, adminContext)).createRsvp;
}