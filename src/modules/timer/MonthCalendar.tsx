import { useLayoutEffect, useRef, useState, type KeyboardEvent } from 'react';
import { ChevronLeft, ChevronRight } from 'lucide-react';
import { IconButton } from '../../components/Primitives';

const monthName = new Intl.DateTimeFormat('de-DE', { month: 'long' });
const dateName = new Intl.DateTimeFormat('de-DE', { dateStyle: 'full' });
const weekdays = ['Mo', 'Di', 'Mi', 'Do', 'Fr', 'Sa', 'So'];

function dayAt(date: Date, day: number, monthOffset = 0) {
  return new Date(date.getFullYear(), date.getMonth() + monthOffset, day, 12);
}

export function MonthCalendar() {
  const [selected, setSelected] = useState(() => new Date());
  const dayButton = useRef<HTMLButtonElement>(null);
  const moveFocus = useRef(false);
  const today = new Date();
  const first = dayAt(selected, 1);
  const leading = (first.getDay() + 6) % 7;
  const count = dayAt(selected, 0, 1).getDate();
  const month = monthName.format(selected);
  const year = selected.getFullYear();

  useLayoutEffect(() => {
    if (!moveFocus.current) return;
    moveFocus.current = false;
    dayButton.current?.focus();
  }, [selected]);

  function changeMonth(offset: number) {
    const days = dayAt(selected, 0, offset + 1).getDate();
    setSelected(dayAt(selected, Math.min(selected.getDate(), days), offset));
  }

  function navigateDays(event: KeyboardEvent<HTMLButtonElement>) {
    const offsets: Record<string, number> = { ArrowLeft: -1, ArrowRight: 1, ArrowUp: -7, ArrowDown: 7 };
    const offset = offsets[event.key];
    if (offset === undefined) return;
    event.preventDefault();
    moveFocus.current = true;
    setSelected(dayAt(selected, selected.getDate() + offset));
  }

  return <section className="month-calendar" aria-label="Monatskalender">
    <header>
      <IconButton icon={ChevronLeft} label="Vorheriger Monat" onClick={() => changeMonth(-1)} />
      <button className="calendar-month" title="Zu heute springen" aria-label={`${month} ${year} – zu heute springen`} onClick={() => setSelected(new Date())}>
        <span aria-live="polite">{month}<i aria-hidden="true" />{year}</span>
      </button>
      <IconButton icon={ChevronRight} label="Nächster Monat" onClick={() => changeMonth(1)} />
    </header>
    <div className="calendar-grid" role="group" aria-label={`${month} ${year}`}>
      {weekdays.map((day) => <span className="calendar-weekday" key={day}>{day}</span>)}
      {Array.from({ length: 42 }, (_, index) => {
        const day = index - leading + 1;
        const date = dayAt(selected, day);
        const adjacent = day < 1 || day > count;
        const current = day === selected.getDate();
        return <button key={day} className={adjacent ? 'calendar-adjacent' : undefined} ref={current ? dayButton : undefined} tabIndex={current ? 0 : -1}
          aria-label={dateName.format(date)} aria-pressed={current} aria-current={date.toDateString() === today.toDateString() ? 'date' : undefined}
          onClick={() => setSelected(date)} onKeyDown={navigateDays}>{date.getDate()}</button>;
      })}
    </div>
  </section>;
}
