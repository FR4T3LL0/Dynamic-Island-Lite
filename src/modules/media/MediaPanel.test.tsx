import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, expect, it, vi } from 'vitest';
import { previewSnapshot } from '../../bridge/preview';
import { perform } from '../../bridge/store';
import { MediaPanel } from './MediaPanel';
import { MediaArtwork } from './MediaArtwork';
vi.mock('../../bridge/store', () => ({ mode: 'preview', perform: vi.fn(async () => true) }));
afterEach(() => { cleanup(); vi.clearAllMocks(); vi.useRealTimers(); });
it('führt nur die sichtbare laufende Zeit fort und beendet die Uhr beim Pausieren oder Schließen', () => {
  vi.useFakeTimers();
  const snapshot=previewSnapshot();snapshot.media!.playing=true;
  const receivedAt=performance.now();const view=render(<MediaPanel snapshot={snapshot} receivedAtMs={receivedAt}/>);
  expect(screen.getByTestId('media-position')).toHaveTextContent('0:58');
  expect(screen.getByTitle('Gesamtdauer')).toHaveTextContent('3:24');
  act(()=>vi.advanceTimersByTime(2000));expect(screen.getByTestId('media-position')).toHaveTextContent('1:00');
  expect(screen.getByTitle('Gesamtdauer')).toHaveTextContent('3:24');
  const paused=structuredClone(snapshot);paused.media!.playing=false;paused.media!.position=60;
  view.rerender(<MediaPanel snapshot={paused} receivedAtMs={performance.now()}/>);
  act(()=>vi.advanceTimersByTime(3000));expect(screen.getByTestId('media-position')).toHaveTextContent('1:00');expect(vi.getTimerCount()).toBe(0);
  view.unmount();expect(vi.getTimerCount()).toBe(0);
});
it('sendet bestätigungsgebundene Aktionen mit Sitzung und Generation; fehlende Fähigkeiten bleiben gesperrt',async()=>{
  const snapshot=previewSnapshot();render(<MediaPanel snapshot={snapshot} receivedAtMs={performance.now()}/>);
  expect(screen.getByRole('button',{name:'Nächster Titel'})).toBeDisabled();
  fireEvent.click(screen.getByRole('button',{name:'Nächster Titel'}));expect(perform).not.toHaveBeenCalled();
  await act(async()=>fireEvent.click(screen.getByRole('button',{name:'Abspielen'})));
  expect(perform).toHaveBeenCalledWith('media_toggle',{id:'preview-player',generation:1});
  expect(screen.getByRole('button',{name:'Abspielen'})).toBeEnabled();
});
it('sendet einen Seek erst beim Abschluss der Eingabe und übernimmt keine unbestätigte Position',async()=>{
  const snapshot=previewSnapshot();render(<MediaPanel snapshot={snapshot} receivedAtMs={performance.now()}/>);
  const slider=screen.getByRole('slider',{name:'Wiedergabeposition'});
  fireEvent.change(slider,{target:{value:'75'}});expect(perform).not.toHaveBeenCalled();
  await act(async()=>fireEvent.keyUp(slider,{key:'ArrowRight'}));
  expect(perform).toHaveBeenCalledWith('media_seek',{id:'preview-player',generation:1,position:75});
  expect(screen.getByTestId('media-position')).toHaveTextContent('0:58');
});
it('behält die bestätigte Quelle bei und zeigt fehlende Titel und Cover verständlich',async()=>{
  const snapshot=previewSnapshot();snapshot.media!.title='';snapshot.media!.artist='';snapshot.media!.cover='http://island-image.localhost/test';
  snapshot.mediaState.sessions.push({...snapshot.media!,id:'second',title:'Zweiter Player'});
  const view=render(<MediaPanel snapshot={snapshot} receivedAtMs={performance.now()}/>);
  expect(screen.getByText('Unbekannter Titel')).toBeVisible();
  const image=view.container.querySelector('img')!;fireEvent.error(image);expect(view.container.querySelector('img')).toBeNull();
  expect(screen.queryByRole('combobox', {name: 'Medienquelle'})).toBeNull();
  expect(perform).not.toHaveBeenCalled();
});

it('zeigt das bisherige Cover, bis das neueste geladen ist, und räumt nach dem Übergang auf', () => {
  const view = render(<MediaArtwork source="/a.png" transition="fade" />);
  const previous = view.container.querySelector('img');
  view.rerender(<MediaArtwork source={null} transition="fade" />);
  expect(view.container.querySelector('.cover-previous img')).toBe(previous);
  expect(view.container.querySelector('.cover-current')).toHaveClass('cover-pending');
  view.rerender(<MediaArtwork source="/b.png" transition="fade" />);
  view.rerender(<MediaArtwork source="/c.png" transition="fade" />);
  expect(view.container.querySelector('.cover-previous img')).toBe(previous);
  expect(view.container.querySelector('.cover-pending img')).toHaveAttribute('src', '/c.png');
  const incoming = view.container.querySelector('.cover-current img')!;
  fireEvent.load(incoming);
  expect(view.container.querySelector('.album-placeholder')).toHaveAttribute('data-cover-transition', 'fade');
  fireEvent.animationEnd(incoming.parentElement!);
  expect(view.container.querySelectorAll('img')).toHaveLength(1);
  view.rerender(<MediaArtwork source="/d.png" transition="off" />);
  expect(view.container.querySelectorAll('img')).toHaveLength(1);
  expect(view.container.querySelector('.album-placeholder')).not.toHaveAttribute('data-cover-transition');
});
