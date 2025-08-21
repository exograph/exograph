import { useAuth0 } from "@auth0/auth0-react";
import { useEffect, useState } from "react";

export function UserIcon() {
  const { getIdTokenClaims } = useAuth0();
  const [picture, setPicture] = useState<string | undefined>();

  useEffect(() => {
    async function getToken() {
      const claims = await getIdTokenClaims();
      setPicture(claims?.picture);
    }

    getToken();
  }, [getIdTokenClaims, setPicture]);

  return <img src={picture} alt="user" width={"20px"} style={{ borderRadius: "50%" }} />;
}
