import type { ButtonHTMLAttributes, ReactNode } from 'react';
import type { LucideIcon } from 'lucide-react';

export function IconButton({ icon: Icon, label, ...props }: ButtonHTMLAttributes<HTMLButtonElement> & { icon: LucideIcon; label: string }) {
  return <button type="button" className="icon-button" aria-label={label} title={label} {...props}><Icon size={18} strokeWidth={1.75} /></button>;
}
export function EmptyState({ icon: Icon, children }: { icon: LucideIcon; children: ReactNode }) {
  return <div className="empty-state"><Icon size={30} strokeWidth={1.3} /><p>{children}</p></div>;
}
