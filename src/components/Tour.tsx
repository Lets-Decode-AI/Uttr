import { useCallback, useEffect, useLayoutEffect, useState } from 'react';
import { markTourDone, tourDone, TOUR_EVENT } from '../lib/tour';
import Button from './ui/Button';

export interface TourStep {
  /** `data-tour` value of the element to point at. */
  target: string;
  title: string;
  body: string;
}

export const TOUR_STEPS: TourStep[] = [
  { target: 'home', title: 'Home', body: 'Your words, speed, streak and the time dictation saved you, plus a checklist to finish setting up.' },
  { target: 'history', title: 'History', body: 'Every dictation is kept on this computer. Search it, filter by app, copy text back, or compare with the original transcript.' },
  { target: 'snippets', title: 'Snippets', body: 'Say a short phrase like “my calendar link” and uttr types the saved text instead.' },
  { target: 'style', title: 'Style', body: 'Formal for email, very casual for chat: pick how your writing sounds in each app.' },
  { target: 'ai', title: 'AI', body: 'Turn on AI cleanup to drop filler words and fix punctuation, and set up Command Mode to rewrite selected text by voice.' },
  { target: 'models', title: 'Models', body: 'Choose how speech becomes text: a model on this computer, or a cloud service with your own API key.' },
  { target: 'settings', title: 'Settings', body: 'Change your shortcut and microphone, how long history is kept, and how uttr starts.' },
  { target: 'status', title: 'You are ready', body: 'This shows when uttr is listening. Hold your shortcut in any app and start talking. You can replay this tour from Home or Settings.' },
];

interface Box { top: number; left: number; width: number; height: number }

/** Coach-mark walkthrough of the main window. Opens on `startTour()`, or by itself once. */
export default function Tour({ autoStart = false }: { autoStart?: boolean }) {
  const [step, setStep] = useState<number | null>(() => (autoStart && !tourDone() ? 0 : null));
  const [box, setBox] = useState<Box | null>(null);

  useEffect(() => {
    const open = () => setStep(0);
    window.addEventListener(TOUR_EVENT, open);
    return () => window.removeEventListener(TOUR_EVENT, open);
  }, []);

  const close = useCallback(() => {
    markTourDone();
    setStep(null);
  }, []);
  const go = useCallback((d: number) => setStep((s) => (s === null ? s : Math.min(Math.max(s + d, 0), TOUR_STEPS.length - 1))), []);

  // Point at the current target: mark it, scroll it into view and measure it.
  useLayoutEffect(() => {
    if (step === null) return;
    const el = document.querySelector<HTMLElement>(`[data-tour="${TOUR_STEPS[step].target}"]`);
    if (!el) { setBox(null); return; }
    el.classList.add('tour-target');
    el.scrollIntoView?.({ block: 'nearest' });
    const measure = () => {
      const r = el.getBoundingClientRect();
      setBox(r.width || r.height ? { top: r.top, left: r.left, width: r.width, height: r.height } : null);
    };
    measure();
    window.addEventListener('resize', measure);
    return () => {
      el.classList.remove('tour-target');
      window.removeEventListener('resize', measure);
    };
  }, [step]);

  useEffect(() => {
    if (step === null) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') close();
      else if (e.key === 'ArrowRight') go(1);
      else if (e.key === 'ArrowLeft') go(-1);
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [step, close, go]);

  if (step === null) return null;
  const s = TOUR_STEPS[step];
  const last = step === TOUR_STEPS.length - 1;
  const pad = 6;
  // The card sits to the right of a sidebar target, or centered when there is none.
  const card = box
    ? { top: Math.max(16, Math.min(box.top - 8, window.innerHeight - 220)), left: box.left + box.width + 18 }
    : undefined;
  return (
    <div className="tour">
      {box
        ? <div className="tour-spot" style={{ top: box.top - pad, left: box.left - pad, width: box.width + pad * 2, height: box.height + pad * 2 }} />
        : <div className="tour-dim" />}
      <div className={`tour-card${box ? '' : ' tour-card-center'}`} style={card} role="dialog" aria-modal="true" aria-label="uttr tour" aria-describedby="tour-body">
        <p className="tour-count">{step + 1} of {TOUR_STEPS.length}</p>
        <h2 className="tour-title">{s.title}</h2>
        <p className="tour-body" id="tour-body">{s.body}</p>
        <div className="tour-dots" aria-hidden="true">{TOUR_STEPS.map((t, i) => <span key={t.target} className={`tour-dot${i === step ? ' tour-dot-on' : ''}`} />)}</div>
        <div className="tour-actions">
          <Button variant="plain" onClick={close}>Skip tour</Button>
          <span className="tour-actions-end">
            {step > 0 && <Button onClick={() => go(-1)}>Back</Button>}
            {last ? <Button variant="primary" autoFocus onClick={close}>Done</Button> : <Button variant="primary" autoFocus onClick={() => go(1)}>Next</Button>}
          </span>
        </div>
      </div>
    </div>
  );
}
