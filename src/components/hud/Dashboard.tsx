import { WeatherWidget } from "../widgets/WeatherWidget";
import { ScheduleWidget } from "../widgets/ScheduleWidget";
// Apple Reminders can't be read via CalDAV once the list is "upgraded" to
// iCloud's new format (which Apple applies to modern accounts and exposes to
// no third-party app), so the main-page Reminders panel uses Jarvis's own
// local reminders — which work and can be filled by voice ("remind me to …").
import { RemindersWidget } from "../widgets/RemindersWidget";

// Left-docked HUD column on the main (orb) screen: weather, Apple Calendar
// schedule, and Apple Reminders. Sits above the orb but off to the side so it
// doesn't obscure the core; scrolls if the content runs long.
export function Dashboard() {
  return (
    <div
      style={{
        position: "absolute",
        top: 64,
        left: 20,
        width: 300,
        maxWidth: "32vw",
        maxHeight: "calc(100vh - 130px)",
        overflowY: "auto",
        display: "flex",
        flexDirection: "column",
        gap: 14,
        zIndex: 5,
      }}
    >
      <WeatherWidget />
      <ScheduleWidget />
      <RemindersWidget />
    </div>
  );
}
