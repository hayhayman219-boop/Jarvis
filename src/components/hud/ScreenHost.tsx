import { SCREENS } from "../../screens/registry";
import { useScreensStore } from "../../state/screensStore";

// Mounts every currently-open screen from the registry. The stack position
// (`index`) is handed to each screen so PopupScreen-based ones cascade.
export function ScreenHost() {
  const open = useScreensStore((s) => s.open);
  const close = useScreensStore((s) => s.closeScreen);

  return (
    <>
      {open.map((id, index) => {
        const def = SCREENS.find((s) => s.id === id);
        if (!def) return null;
        return (
          <div key={id}>{def.render({ onClose: () => close(id), index })}</div>
        );
      })}
    </>
  );
}
