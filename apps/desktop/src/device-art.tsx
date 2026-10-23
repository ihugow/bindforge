import { useId } from "react";

export function MouseArt() {
  const fillId = useId();
  return <svg viewBox="0 0 240 340" aria-hidden="true" className="device-art mouse-art">
    <defs><linearGradient id={fillId} x1="0" y1="0" x2="1" y2="1"><stop stopColor="#344454" /><stop offset="1" stopColor="#121d29" /></linearGradient></defs>
    <path d="M120 18C78 18 53 49 50 99L43 226C40 291 75 322 120 322s80-31 77-96l-7-127C187 49 162 18 120 18Z" fill={`url(#${fillId})`} stroke="#8093aa" strokeWidth="2" />
    <path d="M120 20v103M52 126q68 26 136 0" fill="none" stroke="#8093aa" strokeWidth="1.5" />
    <rect x="110" y="50" width="20" height="47" rx="9" fill="#0d1721" stroke="#87dccc" strokeWidth="2" />
    <path d="M115 61h10m-10 8h10m-10 8h10m-10 8h10" stroke="#617789" />
    <rect x="39" y="144" width="12" height="34" rx="5" fill="#87dccc" /><rect x="36" y="190" width="12" height="34" rx="5" fill="#87dccc" />
    <path d="M93 286q27 13 54 0" fill="none" stroke="#87dccc" strokeWidth="3" />
  </svg>;
}
export function KeyboardArt() {
  return <svg viewBox="0 0 380 180" aria-hidden="true" className="device-art keyboard-art">
    <rect x="8" y="26" width="364" height="134" rx="15" fill="#1e2c3b" stroke="#8093aa" strokeWidth="2" />
    {Array.from({ length: 4 }, (_, row) => Array.from({ length: 12 }, (_, col) => <rect key={`${row}-${col}`} x={22 + col * 28} y={40 + row * 23} width="21" height="16" rx="3" fill="#344454" stroke="#658195" />))}
    <rect x="107" y="136" width="160" height="12" rx="3" fill="#87dccc" />
  </svg>;
}
