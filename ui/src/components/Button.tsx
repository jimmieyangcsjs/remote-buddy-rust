import type { ButtonHTMLAttributes, ReactNode } from "react";

interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: "primary" | "signal" | "ghost" | "danger";
  size?: "sm" | "md";
  icon?: ReactNode;
}

export function Button({
  variant = "ghost",
  size = "md",
  icon,
  children,
  className = "",
  ...props
}: ButtonProps) {
  return (
    <button
      className={`button ${className}`}
      data-variant={variant}
      data-size={size}
      {...props}
    >
      {icon}
      {children}
    </button>
  );
}
