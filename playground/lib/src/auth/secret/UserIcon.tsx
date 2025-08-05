export function UserIcon() {
  return (
    // Specify position to shift the div to the right and down a bit to match the position of the Clerk user icon
    <div style={{ position: "absolute", right: "0px", bottom: "4px" }}>
      <div
        style={{
          width: "20px",
          height: "20px",
          borderRadius: "50%",
          background: "green",
        }}
      ></div>
    </div>
  );
}
