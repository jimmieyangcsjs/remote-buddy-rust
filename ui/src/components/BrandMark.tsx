export function BrandMark({ size = 24 }: { size?: number }) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      aria-hidden="true"
    >
      <rect
        x="6.6"
        y="1.8"
        width="10.8"
        height="20.4"
        rx="5.4"
        stroke="currentColor"
        strokeWidth="1.45"
      />
      <path
        d="M19.2 7.8h.8a1 1 0 0 1 1 1v2.4a1 1 0 0 1-1 1h-.8"
        stroke="currentColor"
        strokeWidth="1.3"
        strokeLinecap="round"
      />
      <circle
        cx="12"
        cy="8.15"
        r="3.15"
        stroke="currentColor"
        strokeWidth="1.15"
        opacity="0.7"
      />
      <circle cx="12" cy="8.15" r="1.12" fill="var(--signal)" />
      <path
        d="M9.3 14.2h5.4"
        stroke="currentColor"
        strokeWidth="1.25"
        strokeLinecap="round"
        opacity="0.75"
      />
      <circle cx="10.05" cy="17.35" r="0.78" fill="currentColor" opacity="0.72" />
      <circle cx="13.95" cy="17.35" r="0.78" fill="var(--signal)" />
    </svg>
  );
}
