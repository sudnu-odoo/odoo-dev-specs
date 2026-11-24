/** The app's line icons: one size and stroke everywhere, coloured by the text. */
const PATHS = {
  compose: "M11.5 2.5l2 2L7 11H5V9zM9 2.5H3.5a1 1 0 0 0-1 1v9a1 1 0 0 0 1 1h9a1 1 0 0 0 1-1V7",
  search: "M7 11.5a4.5 4.5 0 1 0 0-9 4.5 4.5 0 0 0 0 9zM10.3 10.3l3.2 3.2",
  database:
    "M8 2c3 0 5.5.9 5.5 2S11 6 8 6 2.5 5.1 2.5 4 5 2 8 2zM2.5 4v8c0 1.1 2.5 2 5.5 2s5.5-.9 5.5-2V4M2.5 8c0 1.1 2.5 2 5.5 2s5.5-.9 5.5-2",
  settings:
    "M8 10a2 2 0 1 0 0-4 2 2 0 0 0 0 4zM13 8l1.3-1-1.2-2.1-1.6.5-1.3-.8L9.8 3H6.2l-.4 1.6-1.3.8-1.6-.5L1.7 7 3 8l-1.3 1 1.2 2.1 1.6-.5 1.3.8.4 1.6h3.6l.4-1.6 1.3-.8 1.6.5L14.3 9z",
  plus: "M8 3v10M3 8h10",
  folder: "M2 4.5a1 1 0 0 1 1-1h3.2l1.5 1.5H13a1 1 0 0 1 1 1V12a1 1 0 0 1-1 1H3a1 1 0 0 1-1-1z",
  "folder-open":
    "M2 12V4.5a1 1 0 0 1 1-1h3.2l1.5 1.5H12a1 1 0 0 1 1 1V7M2 12l1.6-4.3a1 1 0 0 1 .9-.7H14l-1.8 5.3a1 1 0 0 1-.9.7H3a1 1 0 0 1-1-1z",
  chat: "M3 3.5h10a1 1 0 0 1 1 1v6a1 1 0 0 1-1 1H7l-3 2.5v-2.5H3a1 1 0 0 1-1-1v-6a1 1 0 0 1 1-1z",
  pencil: "M10.5 2.5l3 3L6 13H3v-3z",
  trash: "M3 4.5h10M6.5 4.5V3h3v1.5M4.5 4.5l.6 8.6a1 1 0 0 0 1 .9h3.8a1 1 0 0 0 1-.9l.6-8.6",
} as const;

export type IconName = keyof typeof PATHS;

export function Icon({ name }: { name: IconName }) {
  return (
    <svg className="icon" viewBox="0 0 16 16" width="16" height="16" aria-hidden="true">
      <path
        d={PATHS[name]}
        fill="none"
        stroke="currentColor"
        strokeWidth="1.3"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}
