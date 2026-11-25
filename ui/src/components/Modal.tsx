import { useLayoutEffect, useRef, type CSSProperties, type ReactNode } from "react";

export interface ModalProps {
  label: string;
  /** Escape, a click outside, or the dialog's own buttons. */
  onClose: () => void;
  /** "top" for a search palette; dialogs are centred. */
  place?: "center" | "top";
  /** The widest it gets, e.g. "560px"; it always leaves a margin in small windows. */
  width?: string;
  children: ReactNode;
}

/**
 * A modal dialog on the browser's own `<dialog>`: it keeps the keyboard inside,
 * dims the page behind, closes on Escape, and gives focus back afterwards.
 */
export default function Modal({ label, onClose, place = "center", width = "460px", children }: ModalProps) {
  const ref = useRef<HTMLDialogElement>(null);
  useLayoutEffect(() => {
    const dialog = ref.current;
    const before = document.activeElement as HTMLElement | null;
    if (dialog && !dialog.open) {
      try {
        dialog.showModal();
      } catch {
        // Without showModal (tests, very old engines) it still shows, just not modal.
        dialog.setAttribute("open", "");
      }
      // The browser focuses the first button, which then shows a focus ring. Start
      // typing at once when there is a field; otherwise start on the dialog itself.
      const field = dialog.querySelector<HTMLElement>(
        "input:not([type=checkbox]):not([type=radio]), textarea, select",
      );
      if (field) field.focus();
      else dialog.focus();
    }
    return () => {
      if (dialog?.open) dialog.close?.();
      before?.focus?.();
    };
  }, []);
  return (
    <dialog
      ref={ref}
      tabIndex={-1}
      className="modal"
      data-place={place}
      aria-label={label}
      style={{ "--modal-width": width } as CSSProperties}
      onCancel={(e) => {
        e.preventDefault();
        onClose();
      }}
      // The dialog itself is only hit outside its content: on the dimmed backdrop.
      onMouseDown={(e) => e.target === e.currentTarget && onClose()}
    >
      {children}
    </dialog>
  );
}
