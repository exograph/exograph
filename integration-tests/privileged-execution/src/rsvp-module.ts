import type { ExographPriv } from 'https://deno.land/x/exograph@v0.0.5/index.ts';

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
export async function processRsvp(email: string, count: number, exograph: ExographPriv): Promise<RsvpConfirmation> {
  // Just to test that we can make non-privileged calls through a ExographPriv instance
  const _nonPrivQuery = await exograph.executeQuery('query { __type(name: "Rsvp") { name } }');

  const existing = await exograph.executeQueryPriv(existingRsvpQuery, {
    email,
  }, adminContext);

  if (existing.rsvps.length > 0) {
    const existingRsvp = existing.rsvps[0];

    if (existingRsvp.count !== count) {
      return (await exograph.executeQueryPriv(updateRsvpMutation, {
        id: existingRsvp.id,
        rsvp: {
          email,
          count,
        }
      }, adminContext)).updateRsvp;
    } else {
      return existingRsvp;
    }
  }

  console.log(`No existing RSVP for ${email}`);

  return (await exograph.executeQueryPriv(createRsvpMutation, {
    email,
    count
  }, adminContext)).createRsvp;
}