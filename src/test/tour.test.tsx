import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import Tour, { TOUR_STEPS } from '../components/Tour';
import { startTour, tourDone } from '../lib/tour';

const open = () => act(() => startTour());

beforeEach(() => localStorage.clear());
afterEach(cleanup);

describe('Walkthrough', () => {
  it('opens_on_the_tour_event_and_walks_every_step', () => {
    render(<Tour />);
    expect(screen.queryByRole('dialog')).toBeNull();
    open();
    const dialog = screen.getByRole('dialog', { name: 'uttr tour' });
    expect(dialog.textContent).toContain(`1 of ${TOUR_STEPS.length}`);
    expect(screen.getByRole('heading', { name: TOUR_STEPS[0].title })).toBeTruthy();
    expect(screen.queryByRole('button', { name: 'Back' })).toBeNull();
    for (let i = 1; i < TOUR_STEPS.length; i++) fireEvent.click(screen.getByRole('button', { name: 'Next' }));
    expect(screen.getByRole('heading', { name: TOUR_STEPS[TOUR_STEPS.length - 1].title })).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Done' }));
    expect(screen.queryByRole('dialog')).toBeNull();
    expect(tourDone()).toBe(true);
  });

  it('covers_every_page_of_the_sidebar', () => {
    const targets = TOUR_STEPS.map((s) => s.target);
    for (const t of ['home', 'history', 'snippets', 'style', 'ai', 'models', 'settings', 'status']) expect(targets).toContain(t);
  });

  it('arrow_keys_move_and_escape_closes_as_done', () => {
    render(<Tour />);
    open();
    fireEvent.keyDown(window, { key: 'ArrowRight' });
    expect(screen.getByRole('dialog').textContent).toContain(`2 of ${TOUR_STEPS.length}`);
    fireEvent.keyDown(window, { key: 'ArrowLeft' });
    expect(screen.getByRole('dialog').textContent).toContain(`1 of ${TOUR_STEPS.length}`);
    fireEvent.keyDown(window, { key: 'Escape' });
    expect(screen.queryByRole('dialog')).toBeNull();
    expect(tourDone()).toBe(true);
  });

  it('skip_closes_and_highlights_the_target', () => {
    document.body.insertAdjacentHTML('beforeend', '<a data-tour="home">Home</a>');
    render(<Tour />);
    open();
    expect(document.querySelector('[data-tour="home"]')!.classList.contains('tour-target')).toBe(true);
    fireEvent.click(screen.getByRole('button', { name: 'Skip tour' }));
    expect(screen.queryByRole('dialog')).toBeNull();
    expect(document.querySelector('[data-tour="home"]')!.classList.contains('tour-target')).toBe(false);
    document.querySelector('[data-tour="home"]')!.remove();
  });

  it('auto_starts_once_when_asked', () => {
    const { unmount } = render(<Tour autoStart />);
    expect(screen.getByRole('dialog')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Skip tour' }));
    unmount();
    render(<Tour autoStart />);
    expect(screen.queryByRole('dialog')).toBeNull();
  });
});
