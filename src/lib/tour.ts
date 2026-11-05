/** The in-app walkthrough is started by this window event, so any page can launch it. */
export const TOUR_EVENT = 'uttr:tour';
const DONE_KEY = 'uttr.tour.done';

export function startTour() {
  window.dispatchEvent(new CustomEvent(TOUR_EVENT));
}

export function tourDone(): boolean {
  try {
    return window.localStorage.getItem(DONE_KEY) === '1';
  } catch {
    return false;
  }
}

export function markTourDone() {
  try {
    window.localStorage.setItem(DONE_KEY, '1');
  } catch {
    // Storage can be unavailable; the tour may then show again.
  }
}
