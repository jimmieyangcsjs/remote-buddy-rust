import type { ReactNode } from "react";

export function StatusPill({
  tone,
  children,
  pulse = false,
}: {
  tone: "signal" | "amber" | "coral" | "neutral";
  children: ReactNode;
  pulse?: boolean;
}) {
  return (
    <span className="pill" data-tone={tone}>
      <span className="dot" data-pulse={pulse} />
      {children}
    </span>
  );
}
