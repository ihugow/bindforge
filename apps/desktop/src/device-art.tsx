import { useId } from "react";

export function MouseArt() {
  const id = useId();
  return <svg viewBox="0 0 240 340" aria-hidden="true" className="device-art mouse-art">
    <defs>
      <linearGradient id={`${id}-body`} x1="0" y1="0" x2=".85" y2="1">
        <stop stopColor="var(--device-fill-top)" /><stop offset="1" stopColor="var(--device-fill-bottom)" />
      </linearGradient>
      <linearGradient id={`${id}-buttons`} x1="0" y1="0" x2="0" y2="1">
        <stop stopColor="var(--device-outline)" stopOpacity=".14" /><stop offset="1" stopColor="var(--device-outline)" stopOpacity=".02" />
      </linearGradient>
      <linearGradient id={`${id}-edge`} x1="0" y1="0" x2="1" y2="1">
        <stop stopColor="var(--device-outline)" /><stop offset=".5" stopColor="var(--device-outline)" stopOpacity=".45" /><stop offset="1" stopColor="var(--device-outline)" stopOpacity=".75" />
      </linearGradient>
    </defs>
    <path d="M120 18C94 18 64 32 54 47C46 56 42 77 44 108L48 160C50 183 45 207 44 238C41 288 74 322 120 322C166 322 199 288 196 238C195 207 190 183 192 160L196 108C198 77 194 56 186 47C176 32 146 18 120 18Z"
      fill={`url(#${id}-body)`} stroke={`url(#${id}-edge)`} strokeWidth="2.2" />
    <path d="M116 22C93 23 69 35 58 49C49 72 53 114 59 148Q60 153 67 153H116ZM124 22C147 23 171 35 182 49C191 72 187 114 181 148Q180 153 173 153H124Z"
      fill={`url(#${id}-buttons)`} />
    <path d="M120 20V157M52 55C50 86 54 122 58 148Q59 157 68 157Q120 155 172 157Q181 157 182 148C186 122 190 86 188 55"
      fill="none" stroke="var(--device-outline)" strokeWidth="1.5" strokeOpacity=".65" strokeLinecap="round" />
    <path d="M54 176C55 197 51 216 51 240C48 284 77 313 113 314M186 176C185 197 189 216 189 240C192 284 163 313 127 314"
      fill="none" stroke="var(--device-outline)" strokeWidth="1" strokeOpacity=".15" />
    <rect x="107" y="52" width="26" height="56" rx="13" fill="var(--device-inset)" stroke="var(--device-outline)" strokeOpacity=".55" />
    <rect x="112" y="57" width="16" height="46" rx="8" fill="var(--device-fill-bottom)" stroke="var(--device-accent)" strokeWidth="1.5" />
    <path d="M115 63h10m-10 5h10m-10 5h10m-10 5h10m-10 5h10m-10 5h10m-10 5h10" stroke="var(--device-outline)" strokeOpacity=".75" strokeLinecap="round" />
    <rect x="43" y="123" width="8" height="34" rx="5" fill="var(--device-accent)" stroke="var(--device-fill-bottom)" strokeWidth="1.5" />
    <rect x="43" y="173" width="8" height="34" rx="5" fill="var(--device-accent)" stroke="var(--device-fill-bottom)" strokeWidth="1.5" />
    <circle cx="120" cy="176" r="2.2" fill="var(--device-accent)" />
    <path d="m120 236 4 11 11 4-11 4-4 11-4-11-11-4 11-4Z" fill="none" stroke="var(--device-outline)" strokeWidth="1.5" strokeOpacity=".5" strokeLinejoin="round" />
  </svg>;
}
export function KeyboardArt() {
  return <svg viewBox="0 0 380 180" aria-hidden="true" className="device-art keyboard-art">
    <rect x="8" y="26" width="364" height="134" rx="15" fill="var(--device-fill-bottom)" stroke="var(--device-outline)" strokeWidth="2" />
    {Array.from({ length: 4 }, (_, row) => Array.from({ length: 12 }, (_, col) => <rect key={`${row}-${col}`} x={22 + col * 28} y={40 + row * 23} width="21" height="16" rx="3" fill="var(--device-fill-top)" stroke="var(--device-outline)" />))}
    <rect x="107" y="136" width="160" height="12" rx="3" fill="var(--device-accent)" />
  </svg>;
}
