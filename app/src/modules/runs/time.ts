/** How long ago a run was made, in the largest unit that fits, or its date beyond a week. */
export type Ago =
  | { unit: "now" }
  | { unit: "minutes"; count: number }
  | { unit: "hours"; count: number }
  | { unit: "yesterday" }
  | { unit: "days"; count: number }
  | { unit: "date"; date: string };

const MINUTE = 60_000;
const HOUR = 60 * MINUTE;

/** The time the store wrote, `YYYY-MM-DD HH:MM:SS` by UTC as SQLite writes it, as a date. */
export function storeTime(madeAt: string): Date {
  return new Date(`${madeAt.replace(" ", "T")}Z`);
}

function pad(value: number): string {
  return String(value).padStart(2, "0");
}

/** A date by the local calendar: `YYYY-MM-DD`. */
export function localDate(at: Date): string {
  return `${at.getFullYear()}-${pad(at.getMonth() + 1)}-${pad(at.getDate())}`;
}

/** A time by the local clock, whole: `YYYY-MM-DD HH:MM`. */
export function localTime(at: Date): string {
  return `${localDate(at)} ${pad(at.getHours())}:${pad(at.getMinutes())}`;
}

/** Whole local days from one date's day to another's. */
function daysBetween(from: Date, to: Date): number {
  const day = (at: Date) => Date.UTC(at.getFullYear(), at.getMonth(), at.getDate());
  return Math.round((day(to) - day(from)) / (24 * HOUR));
}

/**
 * How long before `now` a time was: minutes within the hour, hours within the day, `yesterday`
 * for the local day before, days within a week, else its local date.
 */
export function ago(at: Date, now: Date): Ago {
  const elapsed = now.getTime() - at.getTime();
  const days = daysBetween(at, now);
  if (elapsed < MINUTE) {
    return { unit: "now" };
  }
  if (elapsed < HOUR) {
    return { unit: "minutes", count: Math.floor(elapsed / MINUTE) };
  }
  if (days === 0) {
    return { unit: "hours", count: Math.floor(elapsed / HOUR) };
  }
  if (days === 1) {
    return { unit: "yesterday" };
  }
  if (days < 7) {
    return { unit: "days", count: days };
  }
  return { unit: "date", date: localDate(at) };
}
