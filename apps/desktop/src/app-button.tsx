import { forwardRef, type ComponentPropsWithoutRef } from "react";

/** Shared button surface; native props, focus, form behavior and refs are preserved. */
export const AppButton = forwardRef<HTMLButtonElement, ComponentPropsWithoutRef<"button">>(
  function AppButton({ className = "", ...props }, ref) {
    return <button ref={ref} className={`app-button ${className}`.trim()} {...props} />;
  },
);
