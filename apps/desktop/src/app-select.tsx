import { Children, Fragment, isValidElement, useId, useLayoutEffect, useRef, useState, type ReactNode } from "react";
import * as Select from "@radix-ui/react-select";

type OptionProps = { value?: string | number; children?: ReactNode; label?: string; disabled?: boolean };
type AppSelectProps = {
  id: string; value: string | number; onValueChange: (value: string) => void; children: ReactNode;
  disabled?: boolean; required?: boolean; name?: string; className?: string;
  "aria-label"?: string; "aria-describedby"?: string;
};

function renderOptions(children: ReactNode, emptyValue: string, groupDisabled = false): ReactNode {
  return Children.map(children, child => {
    if (!isValidElement<OptionProps>(child)) return null;
    if (child.type === Fragment) return renderOptions(child.props.children, emptyValue, groupDisabled);
    if (child.type === "optgroup") return <Select.Group>
      <Select.Label className="select-group-label">{child.props.label}</Select.Label>
      {renderOptions(child.props.children, emptyValue, groupDisabled || child.props.disabled)}
    </Select.Group>;
    if (child.type !== "option") return null;
    const value = String(child.props.value ?? child.props.children ?? "");
    return <Select.Item className="select-option" value={value || emptyValue} disabled={groupDisabled || child.props.disabled}>
      <Select.ItemText>{child.props.children}</Select.ItemText>
      <Select.ItemIndicator className="select-check" aria-hidden="true">✓</Select.ItemIndicator>
    </Select.Item>;
  });
}

/** Themed menu with native form validation and disabled-fieldset behavior. */
export function AppSelect({ id, value, onValueChange, children, disabled, required, name, className = "", ...aria }: AppSelectProps) {
  const uid = useId();
  const emptyValue = `bindforge-empty-${uid}`;
  const trigger = useRef<HTMLButtonElement>(null);
  const [open, setOpen] = useState(false);
  const [container, setContainer] = useState<HTMLElement | null>(null);
  const [fieldsetDisabled, setFieldsetDisabled] = useState(false);
  const [invalid, setInvalid] = useState(false);
  const unavailable = disabled || fieldsetDisabled;
  useLayoutEffect(() => {
    const button = trigger.current!;
    // Keep the popup inside the native dialog's top layer and focus boundary.
    setContainer(button.closest("dialog") ?? document.body);
    const sync = () => {
      const inherited = button.closest("fieldset:disabled") !== null;
      setFieldsetDisabled(inherited);
      if (inherited) setOpen(false);
    };
    sync();
    const observer = new MutationObserver(sync);
    let ancestor: HTMLElement | null = button.parentElement;
    while (ancestor) {
      if (ancestor.tagName === "FIELDSET") observer.observe(ancestor, { attributes: true, attributeFilter: ["disabled"] });
      ancestor = ancestor.parentElement;
    }
    return () => observer.disconnect();
  }, []);
  const change = (next: string) => { setInvalid(false); onValueChange(next === emptyValue ? "" : next); };
  return <div className="select-control">
    <Select.Root value={String(value) || emptyValue} onValueChange={change} open={open && !unavailable} onOpenChange={setOpen} disabled={unavailable}>
      <Select.Trigger ref={trigger} id={id} className={`select-trigger ${className}`} {...aria} aria-required={required || undefined}
        aria-invalid={invalid || undefined} aria-describedby={[aria["aria-describedby"], invalid ? `${uid}-error` : null].filter(Boolean).join(" ") || undefined}>
        <Select.Value /><Select.Icon className="select-chevron" aria-hidden="true"><svg viewBox="0 0 20 20" width="18" height="18" fill="none"><path d="m5 7.5 5 5 5-5" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" /></svg></Select.Icon>
      </Select.Trigger>
      {container && <Select.Portal container={container}><Select.Content className="select-menu" position="popper" align="start" sideOffset={6} collisionPadding={12}>
        <Select.ScrollUpButton className="select-scroll-button" aria-label="Rolar opções para cima">⌃</Select.ScrollUpButton>
        <Select.Viewport className="select-viewport">{renderOptions(children, emptyValue)}</Select.Viewport>
        <Select.ScrollDownButton className="select-scroll-button" aria-label="Rolar opções para baixo">⌄</Select.ScrollDownButton>
      </Select.Content></Select.Portal>}
    </Select.Root>
    <select aria-hidden="true" tabIndex={-1} name={name} required={required} disabled={unavailable} value={String(value)}
      style={{ position: "absolute", width: 1, height: 1, padding: 0, margin: 0, border: 0, clipPath: "inset(50%)", overflow: "hidden", opacity: 0 }}
      onChange={event => { setInvalid(false); onValueChange(event.target.value); }}
      onInvalid={event => { event.preventDefault(); setInvalid(true); trigger.current?.focus(); }}>
      {children}
    </select>
    {invalid && <span className="select-error" id={`${uid}-error`} role="alert">Selecione uma opção.</span>}
  </div>;
}
