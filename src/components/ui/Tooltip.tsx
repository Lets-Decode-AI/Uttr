import { cloneElement, useEffect, useId, useRef, useState, type ReactElement } from 'react';

export const TOOLTIP_DELAY = 500;

type Trigger = ReactElement<Record<string, unknown>>;

/** Shows `content` above its child after a hover delay, or at once on keyboard focus.
 *  Esc, leaving and blurring hide it. The child gets `aria-describedby` while it shows. */
export default function Tooltip({ content, children }: { content: string; children: Trigger }) {
  const id = useId();
  const [open, setOpen] = useState(false);
  const [below, setBelow] = useState(false);
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const anchor = useRef<HTMLSpanElement>(null);

  const show = (now: boolean) => {
    clearTimeout(timer.current);
    const go = () => {
      // Flip below when there is no room above (near the window's top edge).
      setBelow((anchor.current?.getBoundingClientRect().top ?? 100) < 48);
      setOpen(true);
    };
    if (now) go();
    else timer.current = setTimeout(go, TOOLTIP_DELAY);
  };
  const hide = () => {
    clearTimeout(timer.current);
    setOpen(false);
  };
  useEffect(() => () => clearTimeout(timer.current), []);
  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => { if (e.key === 'Escape') hide(); };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [open]);

  const call = (name: string, e: unknown) => {
    const f = children.props[name];
    if (typeof f === 'function') f(e);
  };
  const trigger = cloneElement(children, {
    'aria-describedby': open ? id : undefined,
    onMouseEnter: (e: unknown) => { call('onMouseEnter', e); show(false); },
    onMouseLeave: (e: unknown) => { call('onMouseLeave', e); hide(); },
    onFocus: (e: unknown) => { call('onFocus', e); show(true); },
    onBlur: (e: unknown) => { call('onBlur', e); hide(); },
  });
  return (
    <span className="ui-tip-anchor" ref={anchor}>
      {trigger}
      {open && <span role="tooltip" id={id} className={`ui-tip${below ? ' ui-tip-below' : ''}`}>{content}</span>}
    </span>
  );
}
