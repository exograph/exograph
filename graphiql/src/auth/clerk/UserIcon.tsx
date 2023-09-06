import { useUser } from "@clerk/clerk-react";

export function UserIcon() {
  const { user } = useUser();

  return (
    <img
      src={user?.imageUrl}
      alt="user"
      width={"20px"}
      style={{ borderRadius: "50%" }}
    />
  );
}
