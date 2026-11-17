import { useEffect, useRef } from "react";
import { X } from "lucide-react";

/** Dialog-Fenster (natives <dialog>): Escape und Schließen-Knopf rufen onClose. */
export function Modal({ title, children, onClose, wide }: { title: string; children: React.ReactNode; onClose: () => void; wide?: boolean }) {
  const ref = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    ref.current?.showModal();
    // Cursor ins erste Eingabefeld statt auf den Schließen-Knopf
    ref.current?.querySelector<HTMLElement>("input:not([type=checkbox]), textarea")?.focus();
  }, []);
  return (
    <dialog ref={ref} className={`dialog ${wide ? "wide" : ""}`} onClose={onClose} onCancel={onClose}>
      <div className="dialog-head">
        <h2>{title}</h2>
        <button className="icon-btn close" onClick={onClose} aria-label="Schließen"><X size={16} /></button>
      </div>
      {children}
    </dialog>
  );
}
