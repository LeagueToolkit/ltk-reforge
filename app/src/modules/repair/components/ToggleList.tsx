import { SectionCard, Switch } from "@/components";

/** One item of a list of things a run turns on and off. */
export interface ToggleItem {
  id: string;
  title: string;
  /** What it rests on: records, a phase. */
  meta: string;
  on: boolean;
  /** Whether the standard set differs from `on`. */
  changed: boolean;
}

export interface ToggleListProps {
  title: string;
  description: string;
  items: ToggleItem[];
  onToggle: (id: string) => void;
}

/** Fixups or checks, each with its switch. One the user changed from the standard is marked. */
export function ToggleList({ title, description, items, onToggle }: ToggleListProps) {
  return (
    <SectionCard title={title} description={description}>
      <ul className="divide-y divide-surface-700/50" data-ui="ToggleList">
        {items.map((item) => (
          <li key={item.id} className="flex items-start justify-between gap-4 py-2">
            <div className="min-w-0">
              <p className="text-sm text-surface-100">
                {item.changed && (
                  <span className="mr-1.5 inline-block h-1.5 w-1.5 rounded-full bg-accent-400 align-middle" />
                )}
                {item.title}
              </p>
              <p className="font-mono text-xs text-surface-400">
                {item.id}
                {item.meta && ` · ${item.meta}`}
              </p>
            </div>
            <Switch
              checked={item.on}
              onCheckedChange={() => onToggle(item.id)}
              aria-label={item.title}
            />
          </li>
        ))}
      </ul>
    </SectionCard>
  );
}
