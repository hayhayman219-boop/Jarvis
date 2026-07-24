const SIZE = 36;
const BORDER = "2px solid var(--jarvis-cyan)";

export function CornerBrackets() {
  return (
    <>
      <div
        style={{
          position: "absolute",
          top: 24,
          left: 24,
          width: SIZE,
          height: SIZE,
          borderTop: BORDER,
          borderLeft: BORDER,
          opacity: 0.6,
        }}
      />
      <div
        style={{
          position: "absolute",
          top: 24,
          right: 24,
          width: SIZE,
          height: SIZE,
          borderTop: BORDER,
          borderRight: BORDER,
          opacity: 0.6,
        }}
      />
      <div
        style={{
          position: "absolute",
          bottom: 24,
          left: 24,
          width: SIZE,
          height: SIZE,
          borderBottom: BORDER,
          borderLeft: BORDER,
          opacity: 0.6,
        }}
      />
      <div
        style={{
          position: "absolute",
          bottom: 24,
          right: 24,
          width: SIZE,
          height: SIZE,
          borderBottom: BORDER,
          borderRight: BORDER,
          opacity: 0.6,
        }}
      />
    </>
  );
}
