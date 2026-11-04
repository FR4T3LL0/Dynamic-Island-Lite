import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, expect, it, vi } from 'vitest';
import { MonthCalendar } from './MonthCalendar';

afterEach(() => { cleanup(); vi.useRealTimers(); });

it('navigiert per Tastatur über den Schalttag und springt zu heute zurück', () => {
  vi.useFakeTimers();
  vi.setSystemTime(new Date(2028, 1, 28, 12));
  render(<MonthCalendar />);
  const day = screen.getByRole('button', { name: /28\. Februar 2028/ });
  day.focus();
  fireEvent.keyDown(day, { key: 'ArrowRight' });
  const leapDay = screen.getByRole('button', { name: /29\. Februar 2028/ });
  expect(leapDay).toHaveFocus();
  fireEvent.keyDown(leapDay, { key: 'ArrowRight' });
  expect(screen.getByRole('button', { name: 'Mittwoch, 1. März 2028' })).toHaveFocus();
  fireEvent.click(screen.getByRole('button', { name: /zu heute springen/ }));
  expect(screen.getByRole('button', { name: /28\. Februar 2028/ })).toHaveAttribute('aria-pressed', 'true');
});
